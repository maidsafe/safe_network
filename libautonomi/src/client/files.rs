use std::{collections::HashMap, path::PathBuf};

use bytes::Bytes;
use serde::{Deserialize, Serialize};
use sn_transfers::HotWallet;
use walkdir::WalkDir;
use xor_name::XorName;

use crate::Client;

use super::data::{GetError, PutError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Root {
    pub map: HashMap<PathBuf, File>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct File {
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
    pub async fn upload_from_dir(
        &mut self,
        path: PathBuf,
        wallet: &mut HotWallet,
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

    pub async fn fetch_root(&mut self, address: XorName) -> Result<Root, UploadError> {
        let data = self.get(address).await?;
        let root: Root = rmp_serde::from_slice(&data[..]).expect("TODO");

        Ok(root)
    }
}

async fn upload_from_file(
    client: &mut Client,
    path: PathBuf,
    wallet: &mut HotWallet,
) -> Result<File, UploadError> {
    let data = tokio::fs::read(path).await?;
    let data = Bytes::from(data);

    let addr = client.put(data, wallet).await?;

    // TODO: Set created_at and modified_at
    Ok(File {
        data_map: addr,
        created_at: 0,
        modified_at: 0,
    })
}
