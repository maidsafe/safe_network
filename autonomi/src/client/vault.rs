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
}

pub trait Vault: ClientWrapper {
    fn with_vault_entropy(self, bytes: Bytes) -> Result<Self, VaultError>
    where
        Self: Sized,
    {
        let client = self.into_client().with_vault_entropy(bytes)?;
        Ok(Self::from_client(client))
    }

    async fn fetch_and_decrypt_vault(&self) -> Result<Option<Bytes>, VaultError> {
        self.client().fetch_and_decrypt_vault().await
    }

    async fn get_vault_from_network(&self) -> Result<Scratchpad, VaultError> {
        self.client().get_vault_from_network().await
    }
}
