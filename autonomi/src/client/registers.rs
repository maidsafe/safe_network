// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

/// Register Secret Key
pub use bls::SecretKey as RegisterSecretKey;
use sn_evm::Amount;
use sn_evm::AttoTokens;
use sn_networking::GetRecordError;
use sn_networking::VerificationKind;
use sn_protocol::storage::RetryStrategy;
pub use sn_registers::{Permissions as RegisterPermissions, RegisterAddress};
use tracing::warn;

use crate::client::data::PayError;
use crate::client::Client;
use bytes::Bytes;
use evmlib::wallet::Wallet;
use libp2p::kad::{Quorum, Record};
use sn_networking::GetRecordCfg;
use sn_networking::NetworkError;
use sn_networking::PutRecordCfg;
use sn_protocol::storage::try_deserialize_record;
use sn_protocol::storage::try_serialize_record;
use sn_protocol::storage::RecordKind;
use sn_protocol::NetworkAddress;
use sn_registers::Register as ClientRegister;
use sn_registers::SignedRegister;
use sn_registers::{EntryHash, Permissions};
use std::collections::BTreeSet;
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
    #[error("Received invalid quote from node, this node is possibly malfunctioning, try another node by trying another register name")]
    InvalidQuote,
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
    /// Generate a new register key
    pub fn register_generate_key() -> RegisterSecretKey {
        RegisterSecretKey::random()
    }

    /// Fetches a Register from the network.
    pub async fn register_get(&self, address: RegisterAddress) -> Result<Register, RegisterError> {
        let network_address = NetworkAddress::from_register_address(address);
        let key = network_address.to_record_key();

        let get_cfg = GetRecordCfg {
            get_quorum: Quorum::Majority,
            retry_strategy: None,
            target_record: None,
            expected_holders: Default::default(),
            is_register: true,
        };

        let register = match self.network.get_record_from_network(key, &get_cfg).await {
            Ok(record) => {
                try_deserialize_record(&record).map_err(|_| RegisterError::Serialization)?
            }
            // manage forked register case
            Err(NetworkError::GetRecordError(GetRecordError::SplitRecord { result_map })) => {
                let mut registers: Vec<SignedRegister> = vec![];
                for (_, (record, _)) in result_map {
                    registers.push(
                        try_deserialize_record(&record)
                            .map_err(|_| RegisterError::Serialization)?,
                    );
                }
                let register = registers.iter().fold(registers[0].clone(), |mut acc, x| {
                    if let Err(e) = acc.merge(x) {
                        warn!("Ignoring forked register as we failed to merge conflicting registers at {}: {e}", x.address());
                    }
                    acc
                });
                register
            }
            Err(e) => Err(e)?,
        };

        // Make sure the fetched record contains valid CRDT operations
        register
            .verify()
            .map_err(|_| RegisterError::FailedVerification)?;

        Ok(Register { inner: register })
    }

    /// Updates a Register on the network with a new value. This will overwrite existing value(s).
    pub async fn register_update(
        &self,
        register: Register,
        new_value: Bytes,
        owner: RegisterSecretKey,
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
            .write(new_value.into(), &children, &owner)
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

        let get_cfg = GetRecordCfg {
            get_quorum: Quorum::Majority,
            retry_strategy: Some(RetryStrategy::default()),
            target_record: None,
            expected_holders: Default::default(),
            is_register: true,
        };
        let put_cfg = PutRecordCfg {
            put_quorum: Quorum::All,
            retry_strategy: None,
            use_put_record_to: None,
            verification: Some((VerificationKind::Network, get_cfg)),
        };

        // Store the updated register on the network
        self.network.put_record(record, &put_cfg).await?;

        Ok(())
    }

    /// Get the cost to create a register
    pub async fn register_cost(
        &self,
        name: String,
        owner: RegisterSecretKey,
    ) -> Result<AttoTokens, RegisterError> {
        // get register address
        let pk = owner.public_key();
        let name = XorName::from_content_parts(&[name.as_bytes()]);
        let permissions = Permissions::new_with([pk]);
        let register = ClientRegister::new(pk, name, permissions);
        let reg_xor = register.address().xorname();

        // get cost to store register
        // NB TODO: register should be priced differently from other data
        let cost_map = self.get_store_quotes(std::iter::once(reg_xor)).await?;
        let total_cost = AttoTokens::from_atto(
            cost_map
                .values()
                .map(|quote| quote.2.cost.as_atto())
                .sum::<Amount>(),
        );

        Ok(total_cost)
    }

    /// Get the address of a register from its name and owner
    pub fn register_address(name: &str, owner: &RegisterSecretKey) -> RegisterAddress {
        let pk = owner.public_key();
        let name = XorName::from_content_parts(&[name.as_bytes()]);
        RegisterAddress::new(name, pk)
    }

    /// Creates a new Register with a name and an initial value and uploads it to the network.
    ///
    /// The Register is created with the owner as the only writer.
    pub async fn register_create(
        &self,
        value: Bytes,
        name: &str,
        owner: RegisterSecretKey,
        wallet: &Wallet,
    ) -> Result<Register, RegisterError> {
        let pk = owner.public_key();
        let permissions = Permissions::new_with([pk]);

        self.register_create_with_permissions(value, name, owner, permissions, wallet)
            .await
    }

    /// Creates a new Register with a name and an initial value and uploads it to the network.
    ///
    /// Unlike `register_create`, this function allows you to specify the permissions for the register.
    pub async fn register_create_with_permissions(
        &self,
        value: Bytes,
        name: &str,
        owner: RegisterSecretKey,
        permissions: RegisterPermissions,
        wallet: &Wallet,
    ) -> Result<Register, RegisterError> {
        let pk = owner.public_key();
        let name = XorName::from_content_parts(&[name.as_bytes()]);

        // Owner can write to the register.
        let mut register = ClientRegister::new(pk, name, permissions);
        let address = NetworkAddress::from_register_address(*register.address());

        let entries = register
            .read()
            .into_iter()
            .map(|(entry_hash, _value)| entry_hash)
            .collect();

        let _ = register.write(value.into(), &entries, &owner);
        let reg_xor = register.address().xorname();
        let (payment_proofs, _skipped) = self.pay(std::iter::once(reg_xor), wallet).await?;
        let proof = if let Some(proof) = payment_proofs.get(&reg_xor) {
            proof
        } else {
            // register was skipped, meaning it was already paid for
            return Err(RegisterError::Network(NetworkError::RegisterAlreadyExists));
        };

        let payee = proof
            .to_peer_id_payee()
            .ok_or(RegisterError::InvalidQuote)?;
        let signed_register = register
            .clone()
            .into_signed(&owner)
            .map_err(RegisterError::CouldNotSign)?;

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

        let get_cfg = GetRecordCfg {
            get_quorum: Quorum::Majority,
            retry_strategy: Some(RetryStrategy::default()),
            target_record: None,
            expected_holders: Default::default(),
            is_register: true,
        };
        let put_cfg = PutRecordCfg {
            put_quorum: Quorum::All,
            retry_strategy: None,
            use_put_record_to: Some(vec![payee]),
            verification: Some((VerificationKind::Network, get_cfg)),
        };

        self.network.put_record(record, &put_cfg).await?;

        Ok(Register {
            inner: signed_register,
        })
    }
}
