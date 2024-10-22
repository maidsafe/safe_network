// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use std::collections::HashSet;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

use crate::client::data::PutError;
use crate::client::Client;
use bls::SecretKey;
use libp2p::kad::{Quorum, Record};
use sn_evm::EvmWallet;
use sn_networking::{GetRecordCfg, NetworkError, PutRecordCfg, VerificationKind};
use sn_protocol::storage::{
    try_serialize_record, RecordKind, RetryStrategy, Scratchpad, ScratchpadAddress,
};
use sn_protocol::Bytes;
use sn_protocol::{storage::try_deserialize_record, NetworkAddress};
use tracing::info;

#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    #[error("Could not generate Vault secret key from entropy: {0:?}")]
    Bls(#[from] bls::Error),
    #[error("Scratchpad found at {0:?} was not a valid record.")]
    CouldNotDeserializeVaultScratchPad(ScratchpadAddress),
    #[error("Protocol: {0}")]
    Protocol(#[from] sn_protocol::Error),
    #[error("Network: {0}")]
    Network(#[from] NetworkError),
}

/// The content type of the vault data
/// The number is used to determine the type of the contents of the bytes contained in a vault
/// Custom apps can use this to store their own custom types of data in vaults
/// It is recommended to use the hash of the app name or an unique identifier as the content type using [`app_name_to_vault_content_type`]
/// The value 0 is reserved for tests
pub type VaultContentType = u64;

/// For custom apps using Scratchpad, this function converts an app identifier or name to a [`VaultContentType`]
pub fn app_name_to_vault_content_type<T: Hash>(s: T) -> VaultContentType {
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

impl Client {
    /// Retrieves and returns a decrypted vault if one exists.
    /// Returns the content type of the bytes in the vault
    pub async fn fetch_and_decrypt_vault(
        &self,
        secret_key: &SecretKey,
    ) -> Result<(Bytes, VaultContentType), VaultError> {
        info!("Fetching and decrypting vault");
        let pad = self.get_vault_from_network(secret_key).await?;

        let data = pad.decrypt_data(secret_key)?;
        Ok((data, pad.data_encoding()))
    }

    /// Gets the vault Scratchpad from a provided client public key
    async fn get_vault_from_network(
        &self,
        secret_key: &SecretKey,
    ) -> Result<Scratchpad, VaultError> {
        let client_pk = secret_key.public_key();

        let scratch_address = ScratchpadAddress::new(client_pk);
        let network_address = NetworkAddress::from_scratchpad_address(scratch_address);
        info!("Fetching vault from network at {network_address:?}",);
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
            .await
            .inspect_err(|err| {
                debug!("Failed to fetch vault {network_address:?} from network: {err}");
            })?;

        let pad = try_deserialize_record::<Scratchpad>(&record)
            .map_err(|_| VaultError::CouldNotDeserializeVaultScratchPad(scratch_address))?;

        Ok(pad)
    }

    /// Put data into the client's VaultPacket
    ///
    /// Pays for a new VaultPacket if none yet created for the client.
    /// Provide the bytes to be written to the vault and the content type of those bytes.
    /// It is recommended to use the hash of the app name or unique identifier as the content type.
    pub async fn write_bytes_to_vault(
        &self,
        data: Bytes,
        wallet: &EvmWallet,
        secret_key: &SecretKey,
        content_type: VaultContentType,
    ) -> Result<(), PutError> {
        let client_pk = secret_key.public_key();

        let pad_res = self.get_vault_from_network(secret_key).await;
        let mut is_new = true;

        let mut scratch = if let Ok(existing_data) = pad_res {
            info!("Scratchpad already exists, returning existing data");

            info!(
                "scratch already exists, is version {:?}",
                existing_data.count()
            );

            is_new = false;
            existing_data
        } else {
            trace!("new scratchpad creation");
            Scratchpad::new(client_pk, content_type)
        };

        let _next_count = scratch.update_and_sign(data, secret_key);
        let scratch_address = scratch.network_address();
        let scratch_key = scratch_address.to_record_key();

        info!("Writing to vault at {scratch_address:?}",);

        let record = if is_new {
            self.pay(
                [&scratch_address].iter().filter_map(|f| f.as_xorname()),
                wallet,
            )
            .await
            .inspect_err(|err| {
                error!("Failed to pay for new vault at addr: {scratch_address:?} : {err}");
            })?;

            let scratch_xor = scratch_address.as_xorname().ok_or(PutError::VaultXorName)?;
            let (payment_proofs, _) = self.pay(std::iter::once(scratch_xor), wallet).await?;
            // Should always be there, else it would have failed on the payment step.
            let proof = payment_proofs.get(&scratch_xor).expect("Missing proof");

            Arc::new(Record {
                key: scratch_key,
                value: try_serialize_record(&(proof, scratch), RecordKind::ScratchpadWithPayment)
                    .map_err(|_| {
                        PutError::Serialization(
                            "Failed to serialize scratchpad with payment".to_string(),
                        )
                    })?
                    .to_vec(),
                publisher: None,
                expires: None,
            })
        } else {
            Arc::new(Record {
                key: scratch_key,
                value: try_serialize_record(&scratch, RecordKind::Scratchpad)
                    .map_err(|_| {
                        PutError::Serialization("Failed to serialize scratchpad".to_string())
                    })?
                    .to_vec(),
                publisher: None,
                expires: None,
            })
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

        debug!("Put record - scratchpad at {scratch_address:?} to the network");
        self.network
            .put_record(record, &put_cfg)
            .await
            .inspect_err(|err| {
                error!(
                    "Failed to put scratchpad {scratch_address:?} to the network with err: {err:?}"
                )
            })?;

        Ok(())
    }
}
