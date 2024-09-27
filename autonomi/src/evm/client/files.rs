use std::{collections::HashMap, path::PathBuf};

use super::data::{GetError, PutError};
use crate::client::files::{FilePointer, Files, Root, UploadError};
use crate::evm::client::EvmClient;
use bytes::{BufMut, Bytes};
use evmlib::wallet::Wallet;
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;
use xor_name::XorName;

impl Files for EvmClient {}

impl EvmClient {
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
}

async fn upload_from_file(
    client: &mut EvmClient,
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
