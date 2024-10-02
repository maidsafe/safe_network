use std::collections::BTreeSet;

use crate::client::data::PayError;
use crate::client::Client;
use bls::SecretKey;
use bytes::Bytes;
use evmlib::wallet::Wallet;
use libp2p::kad::{Quorum, Record};
use sn_networking::GetRecordCfg;
use sn_networking::NetworkError;
use sn_networking::PutRecordCfg;
use sn_protocol::storage::try_deserialize_record;
use sn_protocol::storage::try_serialize_record;
use sn_protocol::storage::RecordKind;
use sn_protocol::storage::RegisterAddress;
use sn_protocol::NetworkAddress;
use sn_registers::Register as ClientRegister;
use sn_registers::SignedRegister;
use sn_registers::{EntryHash, Permissions};
use xor_name::XorName;

#[derive(Debug, thiserror::Error)]
pub enum RegisterError {
    #[error("Network error")]
    Network(#[from] NetworkError),
    #[error("Serialization error")]
    Serialization,
    #[error("Register could not be verified (corrupt)")]
    FailedVerification,
    #[error("Payment failure occurred during register creation.")]
    Pay(#[from] PayError),
    #[error("Failed to retrieve wallet payment")]
    Wallet(#[from] evmlib::wallet::Error),
    #[error("Failed to write to low-level register")]
    Write(#[source] sn_registers::Error),
    #[error("Failed to sign register")]
    CouldNotSign(#[source] sn_registers::Error),
}

#[derive(Clone, Debug)]
pub struct Register {
    pub(crate) inner: SignedRegister,
}

impl Register {
    pub fn address(&self) -> &RegisterAddress {
        self.inner.address()
    }

    /// Retrieve the current values of the register. There can be multiple values
    /// in case a register was updated concurrently. This is because of the nature
    /// of registers, which allows for network concurrency.
    pub fn values(&self) -> Vec<Bytes> {
        self.inner
            .clone()
            .register()
            .expect("register to be valid")
            .read()
            .into_iter()
            .map(|(_hash, value)| value.into())
            .collect()
    }
}

impl Client {
    /// Fetches a Register from the network.
    pub async fn fetch_register(
        &self,
        address: RegisterAddress,
    ) -> Result<Register, RegisterError> {
        let network_address = NetworkAddress::from_register_address(address);
        let key = network_address.to_record_key();

        let get_cfg = GetRecordCfg {
            get_quorum: Quorum::One,
            retry_strategy: None,
            target_record: None,
            expected_holders: Default::default(),
            is_register: true,
        };

        let record = self.network.get_record_from_network(key, &get_cfg).await?;

        let register: SignedRegister =
            try_deserialize_record(&record).map_err(|_| RegisterError::Serialization)?;

        // Make sure the fetched record contains valid CRDT operations
        register
            .verify()
            .map_err(|_| RegisterError::FailedVerification)?;

        Ok(Register { inner: register })
    }

    /// Updates a Register on the network with a new value. This will overwrite existing value(s).
    pub async fn update_register(
        &self,
        register: Register,
        new_value: Bytes,
        owner: SecretKey,
    ) -> Result<(), RegisterError> {
        // Fetch the current register
        let mut signed_register = register.inner;
        let mut register = signed_register
            .clone()
            .register()
            .expect("register to be valid")
            .clone();

        // Get all current branches
        let children: BTreeSet<EntryHash> = register.read().into_iter().map(|(e, _)| e).collect();

        // Write the new value to all branches
        let (_, op) = register
            .write(new_value.to_vec(), &children, &owner)
            .map_err(RegisterError::Write)?;

        // Apply the operation to the register
        signed_register
            .add_op(op.clone())
            .map_err(RegisterError::Write)?;

        // Prepare the record for network storage
        let record = Record {
            key: NetworkAddress::from_register_address(*register.address()).to_record_key(),
            value: try_serialize_record(&signed_register, RecordKind::Register)
                .map_err(|_| RegisterError::Serialization)?
                .to_vec(),
            publisher: None,
            expires: None,
        };

        let put_cfg = PutRecordCfg {
            put_quorum: Quorum::All,
            retry_strategy: None,
            use_put_record_to: None,
            verification: None,
        };

        // Store the updated register on the network
        self.network.put_record(record, &put_cfg).await?;

        Ok(())
    }

    /// Creates a new Register with an initial value and uploads it to the network.
    pub async fn create_register(
        &self,
        value: Bytes,
        name: XorName,
        owner: SecretKey,
        wallet: &Wallet,
    ) -> Result<Register, RegisterError> {
        let pk = owner.public_key();

        // Owner can write to the register.
        let permissions = Permissions::new_with([pk]);
        let mut register = ClientRegister::new(pk, name, permissions);
        let address = NetworkAddress::from_register_address(*register.address());

        let entries = register
            .read()
            .into_iter()
            .map(|(entry_hash, _value)| entry_hash)
            .collect();

        // TODO: Handle error.
        let _ = register.write(value.into(), &entries, &owner);
        let reg_xor = register.address().xorname();
        let (payment_proofs, _) = self.pay(std::iter::once(reg_xor), wallet).await?;
        // Should always be there, else it would have failed on the payment step.
        let proof = payment_proofs.get(&reg_xor).expect("Missing proof");
        let payee = proof.to_peer_id_payee().expect("Missing payee Peer ID");
        let signed_register = register.clone().into_signed(&owner).expect("TODO");

        let record = Record {
            key: address.to_record_key(),
            value: try_serialize_record(
                &(proof, &signed_register),
                RecordKind::RegisterWithPayment,
            )
            .map_err(|_| RegisterError::Serialization)?
            .to_vec(),
            publisher: None,
            expires: None,
        };

        let put_cfg = PutRecordCfg {
            put_quorum: Quorum::All,
            retry_strategy: None,
            use_put_record_to: Some(vec![payee]),
            verification: None,
        };

        self.network.put_record(record, &put_cfg).await?;

        Ok(Register {
            inner: signed_register,
        })
    }
}
