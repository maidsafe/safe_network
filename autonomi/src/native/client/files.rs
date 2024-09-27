use std::{collections::HashMap, path::PathBuf};

use crate::client::files::{FilePointer, Files, Root, UploadError};
use crate::native::client::NativeClient;
use crate::native::Client;
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use sn_transfers::HotWallet;
use walkdir::WalkDir;
use xor_name::XorName;

impl Files for NativeClient {}

impl NativeClient {
    /// Upload a directory to the network. The directory is recursively walked.
    #[cfg(feature = "fs")]
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
        let root_serialized = Bytes::from(rmp_serde::to_vec(&root)?);

        #[cfg(feature = "vault")]
        self.write_bytes_to_vault_if_defined(root_serialized.clone(), wallet)
            .await?;

        let xor_name = self.put(root_serialized, wallet).await?;

        Ok((root, xor_name))
    }
}

async fn upload_from_file(
    client: &mut NativeClient,
    path: PathBuf,
    wallet: &mut HotWallet,
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
