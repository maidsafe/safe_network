use crate::client::data::{GetError, PutError};
use crate::client::Client;
use bytes::Bytes;
use evmlib::wallet::Wallet;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use walkdir::WalkDir;
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
    pub async fn fetch_root(&mut self, address: XorName) -> Result<Root, UploadError> {
        let data = self.get(address).await?;

        Ok(Root::from_bytes(data)?)
    }

    /// Fetch the file pointed to by the given pointer.
    pub async fn fetch_file(&mut self, file: &FilePointer) -> Result<Bytes, UploadError> {
        let data = self.get(file.data_map).await?;
        Ok(data)
    }

    /// Upload a directory to the network. The directory is recursively walked.
    #[cfg(feature = "fs")]
    pub async fn upload_from_dir(
        &mut self,
        path: PathBuf,
        wallet: &Wallet,
    ) -> Result<(Root, XorName), UploadError> {
        let mut map = HashMap::new();

        for entry in WalkDir::new(path) {
            let entry = entry?;

            if !entry.file_type().is_file() {
                continue;
            }

            let path = entry.path().to_path_buf();
            tracing::info!("Uploading file: {path:?}");
            let file = upload_from_file(self, path.clone(), wallet).await?;

            map.insert(path, file);
        }

        let root = Root { map };
        let root_serialized = root.into_bytes()?;

        let xor_name = self.put(root_serialized, wallet).await?;

        Ok((root, xor_name))
    }
}

async fn upload_from_file(
    client: &mut Client,
    path: PathBuf,
    wallet: &Wallet,
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
