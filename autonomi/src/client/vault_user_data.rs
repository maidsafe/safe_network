// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use std::collections::HashMap;
use std::collections::HashSet;

use super::archive::ArchiveAddr;
use super::data::GetError;
use super::data::PutError;
use super::registers::RegisterAddress;
use super::vault::VaultError;
use super::Client;
use crate::client::vault::{app_name_to_vault_content_type, VaultContentType};
use bls::SecretKey;
use serde::{Deserialize, Serialize};
use sn_evm::EvmWallet;
use sn_protocol::Bytes;

use std::sync::LazyLock;

/// Vault content type for UserDataVault
pub static USER_DATA_VAULT_CONTENT_IDENTIFIER: LazyLock<VaultContentType> =
    LazyLock::new(|| app_name_to_vault_content_type("UserData"));

/// UserData is stored in Vaults and contains most of a user's private data:
/// It allows users to keep track of only the key to their User Data Vault
/// while having the rest kept on the Network encrypted in a Vault for them
/// Using User Data Vault is optional, one can decide to keep all their data locally instead.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct UserData {
    /// The register secret key hex encoded
    pub register_sk: Option<String>,
    /// Owned register addresses
    pub registers: HashSet<RegisterAddress>,
    /// Owned file archive addresses
    pub file_archives: HashSet<ArchiveAddr>,

    /// Owner register names, providing it is optional
    pub register_names: HashMap<String, RegisterAddress>,
    /// Owned file archive addresses along with a name for that archive providing it is optional
    pub file_archive_names: HashMap<String, ArchiveAddr>,
}

/// Errors that can occur during the get operation.
#[derive(Debug, thiserror::Error)]
pub enum UserDataVaultGetError {
    #[error("Vault error: {0}")]
    Vault(#[from] VaultError),
    #[error("Unsupported vault content type: {0}")]
    UnsupportedVaultContentType(VaultContentType),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Get error: {0}")]
    GetError(#[from] GetError),
}

impl UserData {
    /// Create a new empty UserData
    pub fn new() -> Self {
        Self::default()
    }

    /// To bytes
    pub fn to_bytes(&self) -> Result<Bytes, rmp_serde::encode::Error> {
        let bytes = rmp_serde::to_vec(&self)?;
        Ok(Bytes::from(bytes))
    }

    /// From bytes
    pub fn from_bytes(bytes: Bytes) -> Result<Self, rmp_serde::decode::Error> {
        let vault_content = rmp_serde::from_slice(&bytes)?;
        Ok(vault_content)
    }
}

impl Client {
    /// Get the user data from the vault
    pub async fn get_user_data_from_vault(
        &self,
        secret_key: &SecretKey,
    ) -> Result<UserData, UserDataVaultGetError> {
        let (bytes, version) = self.fetch_and_decrypt_vault(secret_key).await?;

        if version != *USER_DATA_VAULT_CONTENT_IDENTIFIER {
            return Err(UserDataVaultGetError::UnsupportedVaultContentType(version));
        }

        let vault = UserData::from_bytes(bytes).map_err(|e| {
            UserDataVaultGetError::Serialization(format!(
                "Failed to deserialize vault content: {e}"
            ))
        })?;

        Ok(vault)
    }

    /// Put the user data to the vault
    pub async fn put_user_data_to_vault(
        &self,
        secret_key: &SecretKey,
        wallet: &EvmWallet,
        user_data: UserData,
    ) -> Result<(), PutError> {
        let bytes = user_data
            .to_bytes()
            .map_err(|e| PutError::Serialization(format!("Failed to serialize user data: {e}")))?;
        self.write_bytes_to_vault(
            bytes,
            wallet,
            secret_key,
            *USER_DATA_VAULT_CONTENT_IDENTIFIER,
        )
        .await?;
        Ok(())
    }
}
