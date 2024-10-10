// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::client::data::{GetError, PutError};
use crate::client::Client;
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use xor_name::XorName;

/// Directory-like structure that containing file paths and their metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Root {
    pub map: HashMap<PathBuf, FilePointer>,
}

impl Root {
    /// Deserialize from bytes.
    pub fn from_bytes(data: Bytes) -> Result<Root, rmp_serde::decode::Error> {
        let root: Root = rmp_serde::from_slice(&data[..])?;

        Ok(root)
    }

    /// Serialize to bytes.
    pub fn into_bytes(&self) -> Result<Bytes, rmp_serde::encode::Error> {
        let root_serialized = rmp_serde::to_vec(&self)?;
        let root_serialized = Bytes::from(root_serialized);

        Ok(root_serialized)
    }
}

/// Structure that describes a file on the network. The actual data is stored in
/// chunks, to be constructed with the address pointing to the data map.
///
/// This is similar to ['inodes'](https://en.wikipedia.org/wiki/Inode) in Unix-like filesystems.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilePointer {
    pub(crate) data_map: XorName,
    pub(crate) created_at: u64,
    pub(crate) modified_at: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum UploadError {
    #[error("Failed to recursively traverse directory")]
    WalkDir(#[from] walkdir::Error),
    #[error("Input/output failure")]
    IoError(#[from] std::io::Error),
    #[error("Failed to upload file")]
    PutError(#[from] PutError),
    #[error("Failed to fetch file")]
    GetError(#[from] GetError),
    #[error("Failed to serialize")]
    Serialization(#[from] rmp_serde::encode::Error),
    #[error("Failed to deserialize")]
    Deserialization(#[from] rmp_serde::decode::Error),
}

impl Client {
    /// Fetch a directory from the network.
    pub async fn fetch_root(&self, address: XorName) -> Result<Root, UploadError> {
        let data = self.get(address).await?;

        Ok(Root::from_bytes(data)?)
    }

    /// Fetch the file pointed to by the given pointer.
    pub async fn fetch_file(&self, file: &FilePointer) -> Result<Bytes, UploadError> {
        let data = self.get(file.data_map).await?;
        Ok(data)
    }

    /// Get the cost to upload a file/dir to the network.
    /// quick and dirty implementation, please refactor once files are cleanly implemented
    #[cfg(feature = "fs")]
    pub async fn file_cost(&self, path: &PathBuf) -> Result<sn_evm::AttoTokens, UploadError> {
        let mut map = HashMap::new();
        let mut total_cost = sn_evm::Amount::ZERO;

        for entry in walkdir::WalkDir::new(path) {
            let entry = entry?;

            if !entry.file_type().is_file() {
                continue;
            }

            let path = entry.path().to_path_buf();
            tracing::info!("Cost for file: {path:?}");

            let data = tokio::fs::read(&path).await?;
            let file_bytes = Bytes::from(data);
            let file_cost = self.cost(file_bytes.clone()).await.expect("TODO");

            total_cost += file_cost.as_atto();

            // re-do encryption to get the correct map xorname here
            // this code needs refactor
            let now = sn_networking::target_arch::Instant::now();
            let (data_map_chunk, _) = crate::self_encryption::encrypt(file_bytes).expect("TODO");
            tracing::debug!("Encryption took: {:.2?}", now.elapsed());
            let map_xor_name = *data_map_chunk.address().xorname();
            let data_map_xorname = FilePointer {
                data_map: map_xor_name,
                created_at: 0,
                modified_at: 0,
            };

            map.insert(path, data_map_xorname);
        }

        let root = Root { map };
        let root_serialized = rmp_serde::to_vec(&root).expect("TODO");

        let cost = self.cost(Bytes::from(root_serialized)).await.expect("TODO");
        Ok(cost)
    }

    /// Upload a directory to the network. The directory is recursively walked.
    #[cfg(feature = "fs")]
    pub async fn upload_from_dir(
        &mut self,
        path: PathBuf,
        wallet: &sn_evm::EvmWallet,
    ) -> Result<(Root, XorName), UploadError> {
        let mut map = HashMap::new();

        for entry in walkdir::WalkDir::new(path) {
            let entry = entry?;

            if !entry.file_type().is_file() {
                continue;
            }

            let path = entry.path().to_path_buf();
            tracing::info!("Uploading file: {path:?}");
            println!("Uploading file: {path:?}");
            let file = upload_from_file(self, path.clone(), wallet).await?;

            map.insert(path, file);
        }

        let root = Root { map };
        let root_serialized = root.into_bytes()?;

        let xor_name = self.put(root_serialized, wallet).await?;

        Ok((root, xor_name))
    }
}

#[cfg(feature = "fs")]
async fn upload_from_file(
    client: &mut Client,
    path: PathBuf,
    wallet: &sn_evm::EvmWallet,
) -> Result<FilePointer, UploadError> {
    let data = tokio::fs::read(path).await?;
    let data = Bytes::from(data);

    let addr = client.put(data, wallet).await?;

    // TODO: Set created_at and modified_at
    Ok(FilePointer {
        data_map: addr,
        created_at: 0,
        modified_at: 0,
    })
}
