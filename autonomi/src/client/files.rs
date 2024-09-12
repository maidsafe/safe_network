use std::{collections::HashMap, path::PathBuf};

use super::data::{GetError, PutError};
use crate::Client;
use bytes::Bytes;
use evmlib::wallet::Wallet;
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;
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
    data_map: XorName,
    created_at: u64,
    modified_at: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum UploadError {
    #[error("TODO")]
    WalkDir(#[from] walkdir::Error),
    #[error("TODO")]
    IoError(#[from] std::io::Error),
    #[error("TODO")]
    PutError(#[from] PutError),
    #[error("TODO")]
    GetError(#[from] GetError),
}

impl Client {
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
        let root_serialized = rmp_serde::to_vec(&root).expect("TODO");

        let xor_name = self.put(Bytes::from(root_serialized), wallet).await?;

        Ok((root, xor_name))
    }

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
