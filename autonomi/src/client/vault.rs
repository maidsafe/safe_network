use std::collections::HashSet;

use crate::client::{Client, ClientWrapper};
use bls::SecretKey;
use bytes::Bytes;
use libp2p::kad::Quorum;
use sn_networking::{GetRecordCfg, NetworkError};
use sn_protocol::storage::{Scratchpad, ScratchpadAddress};
use sn_protocol::{storage::try_deserialize_record, NetworkAddress};

#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    #[error("Could not generate Vault secret key from entropy: {0:?}")]
    Bls(#[from] bls::Error),
    #[error("No Vault has been defined. Use `client.with_vault_entropy` to define one.")]
    NoVaultPacketDefined,
    #[error("Scratchpad found at {0:?} was not a valid record.")]
    CouldNotDeserializeVaultScratchPad(ScratchpadAddress),
    #[error("Protocol: {0}")]
    Protocol(#[from] sn_protocol::Error),
    #[error("Network: {0}")]
    Network(#[from] NetworkError),
}

impl Client {
    /// Add a vault secret key to the client
    ///
    /// The secret key is derived from the supplied entropy bytes.
    pub fn with_vault_entropy(mut self, bytes: Bytes) -> Result<Self, VaultError> {
        // simple hash as XORNAME_LEN == SK_LENs
        let xorname = xor_name::XorName::from_content(&bytes);
        // before generating the sk from these bytes.
        self.vault_secret_key = Some(SecretKey::from_bytes(xorname.0)?);

        Ok(self)
    }

    /// Retrieves and returns a decrypted vault if one exists.
    pub async fn fetch_and_decrypt_vault(&self) -> Result<Option<Bytes>, VaultError> {
        let Some(vault_secret_key) = self.vault_secret_key.as_ref() else {
            return Err(VaultError::NoVaultPacketDefined);
        };

        let pad = self.get_vault_from_network().await?;

        Ok(pad.decrypt_data(vault_secret_key)?)
    }

    /// Gets the vault Scratchpad from a provided client public key
    async fn get_vault_from_network(&self) -> Result<Scratchpad, VaultError> {
        // let vault = self.vault.as_ref()?;
        let Some(vault_secret_key) = self.vault_secret_key.as_ref() else {
            return Err(VaultError::NoVaultPacketDefined);
        };

        let client_pk = vault_secret_key.public_key();

        let scratch_address = ScratchpadAddress::new(client_pk);
        let network_address = NetworkAddress::from_scratchpad_address(scratch_address);
        let scratch_key = network_address.to_record_key();

        let get_cfg = GetRecordCfg {
            get_quorum: Quorum::Majority,
            retry_strategy: None,
            target_record: None,
            expected_holders: HashSet::new(),
            is_register: false,
        };

        let record = self
            .network
            .get_record_from_network(scratch_key, &get_cfg)
            .await?;

        let pad = try_deserialize_record::<Scratchpad>(&record)
            .map_err(|_| VaultError::CouldNotDeserializeVaultScratchPad(scratch_address))?;

        Ok(pad)
    }

    /// Put data into the client's VaultPacket
    ///
    /// Returns Ok(None) early if no vault packet is defined.
    ///
    /// Pays for a new VaultPacket if none yet created for the client. Returns the current version
    /// of the data on success.
    pub async fn write_bytes_to_vault_if_defined(
        &mut self,
        data: Bytes,
        wallet: &mut Wallet,
    ) -> Result<Option<u64>, PutError> {
        // Exit early if no vault packet defined
        let Some(client_sk) = self.client().vault_secret_key.as_ref() else {
            return Ok(None);
        };

        let client_pk = client_sk.public_key();

        let pad_res = self.get_vault_from_network().await;
        let mut is_new = true;

        let mut scratch = if let Ok(existing_data) = pad_res {
            tracing::info!("Scratchpad already exists, returning existing data");

            info!(
                "scratch already exists, is version {:?}",
                existing_data.count()
            );

            is_new = false;
            existing_data
        } else {
            tracing::trace!("new scratchpad creation");
            Scratchpad::new(client_pk)
        };

        let next_count = scratch.update_and_sign(data, client_sk);
        let scratch_address = scratch.network_address();
        let scratch_key = scratch_address.to_record_key();

        let record = if is_new {
            self.pay(
                [&scratch_address].iter().filter_map(|f| f.as_xorname()),
                wallet,
            )
            .await?;

            let scratch_xor = scratch_address.as_xorname().ok_or(PutError::VaultXorName)?;
            let (payment_proofs, _) = self.pay(std::iter::once(scratch_xor), wallet).await?;
            // Should always be there, else it would have failed on the payment step.
            let proof = payment_proofs.get(&scratch_xor).expect("Missing proof");

            Record {
                key: scratch_key,
                value: try_serialize_record(&(proof, scratch), RecordKind::ScratchpadWithPayment)
                    .map_err(|_| PutError::Serialization)?
                    .to_vec(),
                publisher: None,
                expires: None,
            }
        } else {
            Record {
                key: scratch_key,
                value: try_serialize_record(&scratch, RecordKind::Scratchpad)
                    .map_err(|_| PutError::Serialization)?
                    .to_vec(),
                publisher: None,
                expires: None,
            }
        };

        let put_cfg = PutRecordCfg {
            put_quorum: Quorum::Majority,
            retry_strategy: Some(RetryStrategy::Balanced),
            use_put_record_to: None,
            verification: Some((
                VerificationKind::Network,
                GetRecordCfg {
                    get_quorum: Quorum::Majority,
                    retry_strategy: None,
                    target_record: None,
                    expected_holders: HashSet::new(),
                    is_register: false,
                },
            )),
        };

        self.network().put_record(record, &put_cfg).await?;

        Ok(Some(next_count))
    }
}
