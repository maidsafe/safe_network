use crate::client::data::{GetError, PutError};
use crate::client::{Client, ClientWrapper};
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
        let root: Root = rmp_serde::from_slice(&data[..]).expect("TODO");

        Ok(root)
    }

    /// Fetch the file pointed to by the given pointer.
    pub async fn fetch_file(&mut self, file: &FilePointer) -> Result<Bytes, UploadError> {
        let data = self.get(file.data_map).await?;
        Ok(data)
    }
}

pub trait Files: ClientWrapper {
    async fn fetch_root(&mut self, address: XorName) -> Result<Root, UploadError> {
        self.client_mut().fetch_root(address).await
    }

    async fn fetch_file(&mut self, file: &FilePointer) -> Result<Bytes, UploadError> {
        self.client_mut().fetch_file(file).await
    }
}
