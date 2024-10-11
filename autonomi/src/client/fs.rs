// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::client::Client;
use bytes::Bytes;
use sn_evm::EvmWallet;
use std::collections::HashMap;
use std::path::PathBuf;

use super::archive::{Archive, ArchiveAddr};
use super::data::{DataAddr, GetError, PutError};

/// Errors that can occur during the file upload operation.
#[cfg(feature = "fs")]
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

#[cfg(feature = "fs")]
/// Errors that can occur during the download operation.
#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("Failed to download file")]
    GetError(#[from] GetError),
    #[error("IO failure")]
    IoError(#[from] std::io::Error),
}

impl Client {
    /// Download file from network to local file system
    pub async fn file_download(
        &self,
        data_addr: DataAddr,
        to_dest: PathBuf,
    ) -> Result<(), DownloadError> {
        let data = self.data_get(data_addr).await?;
        if let Some(parent) = to_dest.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(to_dest, data).await?;
        Ok(())
    }

    /// Download directory from network to local file system
    pub async fn dir_download(
        &self,
        archive_addr: ArchiveAddr,
        to_dest: PathBuf,
    ) -> Result<(), DownloadError> {
        let archive = self.archive_get(archive_addr).await?;
        for (path, addr) in archive.map {
            self.file_download(addr, to_dest.join(path)).await?;
        }
        Ok(())
    }

    /// Upload a directory to the network. The directory is recursively walked.
    /// Reads all files, splits into chunks, uploads chunks, uploads datamaps, uploads archive, returns ArchiveAddr (pointing to the archive)
    pub async fn dir_upload(
        &mut self,
        dir_path: PathBuf,
        wallet: &EvmWallet,
    ) -> Result<ArchiveAddr, UploadError> {
        let mut map = HashMap::new();

        for entry in walkdir::WalkDir::new(dir_path) {
            let entry = entry?;

            if !entry.file_type().is_file() {
                continue;
            }

            let path = entry.path().to_path_buf();
            tracing::info!("Uploading file: {path:?}");
            #[cfg(feature = "loud")]
            println!("Uploading file: {path:?}");
            let file = self.file_upload(path.clone(), wallet).await?;

            map.insert(path, file);
        }

        let archive = Archive { map };
        let archive_serialized = archive.into_bytes()?;

        let arch_addr = self.data_put(archive_serialized, wallet).await?;

        Ok(arch_addr)
    }

    /// Upload a file to the network.
    /// Reads file, splits into chunks, uploads chunks, uploads datamap, returns DataAddr (pointing to the datamap)
    async fn file_upload(
        &mut self,
        path: PathBuf,
        wallet: &EvmWallet,
    ) -> Result<DataAddr, UploadError> {
        let data = tokio::fs::read(path).await?;
        let data = Bytes::from(data);
        let addr = self.data_put(data, wallet).await?;
        Ok(addr)
    }

    /// Get the cost to upload a file/dir to the network.
    /// quick and dirty implementation, please refactor once files are cleanly implemented
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
            let file_cost = self.data_cost(file_bytes.clone()).await.expect("TODO");

            total_cost += file_cost.as_atto();

            // re-do encryption to get the correct map xorname here
            // this code needs refactor
            let now = sn_networking::target_arch::Instant::now();
            let (data_map_chunk, _) = crate::self_encryption::encrypt(file_bytes).expect("TODO");
            tracing::debug!("Encryption took: {:.2?}", now.elapsed());
            let map_xor_name = *data_map_chunk.address().xorname();

            map.insert(path, map_xor_name);
        }

        let root = Archive { map };
        let root_serialized = rmp_serde::to_vec(&root).expect("TODO");

        let archive_cost = self
            .data_cost(Bytes::from(root_serialized))
            .await
            .expect("TODO");

        total_cost += archive_cost.as_atto();
        Ok(total_cost.into())
    }
}
