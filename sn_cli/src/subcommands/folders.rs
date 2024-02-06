// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::files::{upload_files, ChunkManager};

use sn_client::{Client, FolderEntry, FoldersApi, BATCH_SIZE, MAX_UPLOAD_RETRIES};
use sn_registers::RegisterAddress;

use clap::Parser;
use color_eyre::Result;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

#[derive(Parser, Debug)]
pub enum FoldersCmds {
    Upload {
        /// The location of the file(s) to upload for creating the folder on the network.
        ///
        /// Can be a file or a directory.
        #[clap(name = "path", value_name = "PATH")]
        path: PathBuf,
        /// The batch_size to split chunks into parallel handling batches
        /// during payment and upload processing.
        #[clap(long, default_value_t = BATCH_SIZE, short='b')]
        batch_size: usize,
        /// Should the file be made accessible to all. (This is irreversible)
        #[clap(long, name = "make_public", default_value = "false", short = 'p')]
        make_public: bool,
        /// The retry_count for retrying failed chunks
        /// during payment and upload processing.
        #[clap(long, default_value_t = MAX_UPLOAD_RETRIES, short = 'r')]
        max_retries: usize,
    },
    Download {
        /// The hex address of a folder.
        #[clap(name = "address")]
        folder_addr: String,
        /// The name to apply to the downloaded folder.
        #[clap(name = "target name")]
        folder_name: String,
        /// The batch_size for parallel downloading
        #[clap(long, default_value_t = BATCH_SIZE , short='b')]
        batch_size: usize,
    },
}

pub(crate) async fn folders_cmds(
    cmds: FoldersCmds,
    client: &Client,
    root_dir: &Path,
    verify_store: bool,
) -> Result<()> {
    match cmds {
        FoldersCmds::Upload {
            path,
            batch_size,
            max_retries,
            make_public,
        } => {
            upload_files(
                path.clone(),
                make_public,
                client,
                root_dir.to_path_buf(),
                verify_store,
                batch_size,
                max_retries,
            )
            .await?;

            let mut chunk_manager = ChunkManager::new(root_dir);
            chunk_manager.chunk_path(&path, true, make_public)?;

            let mut dirs_paths = BTreeMap::<PathBuf, FoldersApi>::new();
            for (dir_path, parent, dir_name) in WalkDir::new(&path)
                .into_iter()
                .filter_entry(|e| e.file_type().is_dir())
                .flatten()
                .filter(|e| e.depth() > 0)
                .filter_map(|e| {
                    e.path()
                        .parent()
                        .zip(e.file_name().to_str())
                        .map(|(p, n)| (e.path().to_path_buf(), p.to_owned(), n.to_owned()))
                })
            {
                let curr_folder_addr = *dirs_paths
                    .entry(dir_path)
                    .or_insert(FoldersApi::new(client.clone(), root_dir))
                    .address();

                let parent_folder = dirs_paths
                    .entry(parent)
                    .or_insert(FoldersApi::new(client.clone(), root_dir));
                parent_folder.add_folder(dir_name, curr_folder_addr)?;
            }

            for chunked_file in chunk_manager.iter_chunked_files() {
                if let (Some(file_name), Some(parent)) = (
                    chunked_file.file_name.to_str(),
                    chunked_file.file_path.parent(),
                ) {
                    if let Some(folder) = dirs_paths.get_mut(parent) {
                        folder.add_file(file_name.to_string(), chunked_file.head_chunk_address)?;
                    }
                }
            }

            // TODO: sync Folders concurrently
            for (path, mut folder) in dirs_paths {
                let address = folder.sync(verify_store).await?;
                println!(
                    "Folder (for {}) synced with the network at: {}",
                    path.display(),
                    address.to_hex()
                );
            }
        }
        FoldersCmds::Download {
            folder_addr,
            folder_name,
            batch_size,
        } => {
            let address =
                RegisterAddress::from_hex(&folder_addr).expect("Failed to parse Folder address");

            let download_dir = dirs_next::download_dir().unwrap_or(root_dir.to_path_buf());
            let downloaded_folder_path = download_dir.join(folder_name);
            println!(
                "Downloading onto {downloaded_folder_path:?} from {} with batch-size {batch_size}",
                address.to_hex()
            );
            debug!(
                "Downloading onto {downloaded_folder_path:?} from {}",
                address.to_hex()
            );

            let folders_api = FoldersApi::retrieve(client.clone(), root_dir, address).await?;
            println!("Entries in downloaded Folder:");
            folders_api
                .files()?
                .iter()
                .for_each(|(file_name, folder_entry)| match folder_entry {
                    FolderEntry::File(addr) => println!("file- {file_name}: {addr:?}"),
                    FolderEntry::Folder(addr) => println!("subfolder- {file_name}: {addr}"),
                });
        }
    };
    Ok(())
}
