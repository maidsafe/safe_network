use crate::client::registers::{Register, RegisterError, Registers};
use crate::client::ClientWrapper;
use crate::native::client::NativeClient;
use bls::SecretKey;
use bytes::Bytes;
use libp2p::kad::{Quorum, Record};
use sn_networking::PutRecordCfg;
use sn_protocol::storage::try_serialize_record;
use sn_protocol::storage::RecordKind;
use sn_protocol::NetworkAddress;
use sn_registers::Permissions;
use sn_registers::Register as ClientRegister;
use sn_transfers::HotWallet;
use xor_name::XorName;

impl Registers for NativeClient {}

impl NativeClient {
    /// Creates a new Register with an initial value and uploads it to the network.
    pub async fn create_register(
        &mut self,
        value: Bytes,
        name: XorName,
        owner: SecretKey,
        wallet: &mut HotWallet,
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
        register
            .write(value.into(), &entries, &owner)
            .map_err(RegisterError::Write)?;

        let _payment_result = self
            .pay(std::iter::once(register.address().xorname()), wallet)
            .await?;

        let (payment, payee) =
            self.get_recent_payment_for_addr(&register.address().xorname(), wallet)?;

        let signed_register = register
            .clone()
            .into_signed(&owner)
            .map_err(RegisterError::CouldNotSign)?;

        let record = Record {
            key: address.to_record_key(),
            value: try_serialize_record(
                &(payment, &signed_register),
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

        self.network().put_record(record, &put_cfg).await?;

        Ok(Register {
            inner: signed_register,
        })
    }
}
