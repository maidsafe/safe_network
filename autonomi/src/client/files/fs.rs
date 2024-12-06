// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::client::data::{CostError, DataMapChunk, GetError, PutError};
use crate::client::utils::process_tasks_with_max_concurrency;
use crate::client::Client;
use ant_evm::EvmWallet;
use bytes::Bytes;
use std::{path::PathBuf, sync::LazyLock};

use super::archive::{PrivateArchive, PrivateArchiveAccess};

/// Number of files to upload in parallel.
///
/// Can be overridden by the `FILE_UPLOAD_BATCH_SIZE` environment variable.
pub static FILE_UPLOAD_BATCH_SIZE: LazyLock<usize> = LazyLock::new(|| {
    let batch_size = std::env::var("FILE_UPLOAD_BATCH_SIZE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1)
                * 8,
        );
    info!("File upload batch size: {}", batch_size);
    batch_size
});

/// Errors that can occur during the file upload operation.
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

/// Errors that can occur during the download operation.
#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("Failed to download file")]
    GetError(#[from] GetError),
    #[error("IO failure")]
    IoError(#[from] std::io::Error),
}

/// Errors that can occur during the file cost calculation.
#[derive(Debug, thiserror::Error)]
pub enum FileCostError {
    #[error("Cost error: {0}")]
    Cost(#[from] CostError),
    #[error("IO failure")]
    IoError(#[from] std::io::Error),
    #[error("Serialization error")]
    Serialization(#[from] rmp_serde::encode::Error),
    #[error("Self encryption error")]
    SelfEncryption(#[from] crate::self_encryption::Error),
    #[error("Walkdir error")]
    WalkDir(#[from] walkdir::Error),
}

impl Client {
    /// Download a private file from network to local file system
    pub async fn file_download(
        &self,
        data_access: DataMapChunk,
        to_dest: PathBuf,
    ) -> Result<(), DownloadError> {
        let data = self.data_get(data_access).await?;
        if let Some(parent) = to_dest.parent() {
            tokio::fs::create_dir_all(parent).await?;
            debug!("Created parent directories for {to_dest:?}");
        }
        tokio::fs::write(to_dest.clone(), data).await?;
        debug!("Downloaded file to {to_dest:?}");
        Ok(())
    }

    /// Download a private directory from network to local file system
    pub async fn dir_download(
        &self,
        archive_access: PrivateArchiveAccess,
        to_dest: PathBuf,
    ) -> Result<(), DownloadError> {
        let archive = self.archive_get(archive_access).await?;
        for (path, addr, _meta) in archive.iter() {
            self.file_download(addr.clone(), to_dest.join(path)).await?;
        }
        debug!("Downloaded directory to {to_dest:?}");
        Ok(())
    }

    /// Upload a directory to the network. The directory is recursively walked and each file is uploaded to the network.
    /// The data maps of these (private) files are not uploaded but returned within the [`PrivateArchive`] return type.
    pub async fn dir_upload(
        &self,
        dir_path: PathBuf,
        wallet: &EvmWallet,
    ) -> Result<PrivateArchive, UploadError> {
        info!("Uploading directory as private: {dir_path:?}");
        let start = tokio::time::Instant::now();

        // start upload of file in parallel
        let mut upload_tasks = Vec::new();
        for entry in walkdir::WalkDir::new(dir_path) {
            let entry = entry?;
            if !entry.file_type().is_file() {
                continue;
            }

            let metadata = super::fs_public::metadata_from_entry(&entry);
            let path = entry.path().to_path_buf();
            upload_tasks.push(async move {
                let file = self.file_upload(path.clone(), wallet).await;
                (path, metadata, file)
            });
        }

        // wait for all files to be uploaded
        let uploads =
            process_tasks_with_max_concurrency(upload_tasks, *FILE_UPLOAD_BATCH_SIZE).await;
        info!(
            "Upload of {} files completed in {:?}",
            uploads.len(),
            start.elapsed()
        );
        let mut archive = PrivateArchive::new();
        for (path, metadata, maybe_file) in uploads.into_iter() {
            match maybe_file {
                Ok(file) => archive.add_file(path, file, metadata),
                Err(err) => {
                    error!("Failed to upload file: {path:?}: {err:?}");
                    return Err(err);
                }
            }
        }

        #[cfg(feature = "loud")]
        println!("Upload completed in {:?}", start.elapsed());
        Ok(archive)
    }

    /// Same as [`Client::dir_upload`] but also uploads the archive (privately) to the network.
    ///
    /// Returns the [`PrivateArchiveAccess`] allowing the private archive to be downloaded from the network.
    pub async fn dir_and_archive_upload(
        &self,
        dir_path: PathBuf,
        wallet: &EvmWallet,
    ) -> Result<PrivateArchiveAccess, UploadError> {
        let archive = self.dir_upload(dir_path, wallet).await?;
        let archive_addr = self.archive_put(archive, wallet.into()).await?;
        Ok(archive_addr)
    }

    /// Upload a private file to the network.
    /// Reads file, splits into chunks, uploads chunks, uploads datamap, returns [`DataMapChunk`] (pointing to the datamap)
    async fn file_upload(
        &self,
        path: PathBuf,
        wallet: &EvmWallet,
    ) -> Result<DataMapChunk, UploadError> {
        info!("Uploading file: {path:?}");
        #[cfg(feature = "loud")]
        println!("Uploading file: {path:?}");

        let data = tokio::fs::read(path).await?;
        let data = Bytes::from(data);
        let addr = self.data_put(data, wallet.into()).await?;
        debug!("Uploaded file successfully in the privateAchive: {addr:?}");
        Ok(addr)
    }
}
