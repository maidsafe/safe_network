// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::files::{download_file, upload_files, ChunkManager, UploadedFile, UPLOADED_FILES};

use sn_client::{Client, FilesApi, FolderEntry, FoldersApi, WalletClient, BATCH_SIZE};

use sn_protocol::storage::{Chunk, ChunkAddress, RegisterAddress, RetryStrategy};
use sn_transfers::HotWallet;

use clap::Parser;
use color_eyre::{
    eyre::{bail, eyre},
    Result,
};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs::create_dir_all,
    path::{Path, PathBuf},
};
use tokio::task::JoinSet;
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
        /// Set the strategy to use on chunk upload failure. Does not modify the spend failure retry attempts yet.
        ///
        /// Choose a retry strategy based on effort level, from 'quick' (least effort), through 'balanced',
        /// to 'persistent' (most effort).
        #[clap(long, default_value_t = RetryStrategy::Balanced, short = 'r', help = "Sets the retry strategy on upload failure. Options: 'quick' for minimal effort, 'balanced' for moderate effort, or 'persistent' for maximum effort.")]
        retry_strategy: RetryStrategy,
    },
    Download {
        /// The hex address of a folder.
        #[clap(name = "address")]
        folder_addr: String,
        /// The name to apply to the downloaded folder.
        #[clap(name = "target folder name")]
        folder_name: OsString,
        /// The batch_size for parallel downloading
        #[clap(long, default_value_t = BATCH_SIZE , short='b')]
        batch_size: usize,
        /// Set the strategy to use on downloads failure.
        ///
        /// Choose a retry strategy based on effort level, from 'quick' (least effort), through 'balanced',
        /// to 'persistent' (most effort).
        #[clap(long, default_value_t = RetryStrategy::Quick, short = 'r', help = "Sets the retry strategy on download failure. Options: 'quick' for minimal effort, 'balanced' for moderate effort, or 'persistent' for maximum effort.")]
        retry_strategy: RetryStrategy,
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
            make_public,
            retry_strategy,
        } => {
            upload_files(
                path.clone(),
                make_public,
                client,
                root_dir.to_path_buf(),
                verify_store,
                batch_size,
                retry_strategy,
            )
            .await?;

            let mut chunk_manager = ChunkManager::new(root_dir);
            chunk_manager.chunk_path(&path, true, make_public)?;

            let mut folders = build_folders_hierarchy(&path, client, root_dir)?;

            // add chunked files to the corresponding Folders
            for chunked_file in chunk_manager.iter_chunked_files() {
                if let Some(parent) = chunked_file.file_path.parent() {
                    if let Some(folder) = folders.get_mut(parent) {
                        folder.add_file(
                            chunked_file.file_name.clone(),
                            chunked_file.head_chunk_address,
                        )?;
                    }
                }
            }

            println!("Paying for folders hierarchy and uploading...");
            let root_dir_address = folders
                .get(&path)
                .map(|folder| *folder.address())
                .ok_or(eyre!("Failed to obtain main Folder network address"))?;

            pay_and_upload_folders(folders, verify_store, client, root_dir).await?;

            println!(
                "\nFolder hierarchy from {path:?} uploaded successfully at {}",
                root_dir_address.to_hex()
            );
        }
        FoldersCmds::Download {
            folder_addr,
            folder_name,
            batch_size,
            retry_strategy,
        } => {
            let address =
                RegisterAddress::from_hex(&folder_addr).expect("Failed to parse Folder address");

            let download_dir = dirs_next::download_dir().unwrap_or(root_dir.to_path_buf());
            let download_folder_path = download_dir.join(folder_name.clone());
            println!(
                "Downloading onto {download_folder_path:?} from {} with batch-size {batch_size}",
                address.to_hex()
            );
            debug!(
                "Downloading onto {download_folder_path:?} from {}",
                address.to_hex()
            );

            let mut files_to_download = vec![];
            let mut folders_to_download =
                vec![(folder_name, address, download_folder_path.clone())];

            while let Some((name, folder_addr, target_path)) = folders_to_download.pop() {
                if !name.is_empty() {
                    println!(
                        "Downloading Folder {name:?} from {}",
                        hex::encode(folder_addr.xorname())
                    );
                }
                download_folder(
                    root_dir,
                    client,
                    &target_path,
                    folder_addr,
                    &mut files_to_download,
                    &mut folders_to_download,
                )
                .await?;
            }

            let files_api: FilesApi = FilesApi::new(client.clone(), download_folder_path);
            let uploaded_files_path = root_dir.join(UPLOADED_FILES);
            for (file_name, addr, path) in files_to_download {
                // try to read the data_map if it exists locally.
                let expected_data_map_location = uploaded_files_path.join(addr.to_hex());
                let local_data_map = UploadedFile::read(&expected_data_map_location)
                    .map(|uploaded_file_metadata| {
                        uploaded_file_metadata.data_map.map(|bytes| Chunk {
                            address: ChunkAddress::new(*addr.xorname()),
                            value: bytes,
                        })
                    })
                    .unwrap_or(None);

                download_file(
                    files_api.clone(),
                    *addr.xorname(),
                    (file_name, local_data_map),
                    &path,
                    false,
                    batch_size,
                    retry_strategy,
                )
                .await;
            }
        }
    };
    Ok(())
}

// Build Folders hierarchy from the provided disk path
fn build_folders_hierarchy(
    path: &Path,
    client: &Client,
    root_dir: &Path,
) -> Result<BTreeMap<PathBuf, FoldersApi>> {
    let mut folders = BTreeMap::new();
    for (dir_path, depth, parent, dir_name) in WalkDir::new(path)
        .into_iter()
        .filter_entry(|e| e.file_type().is_dir())
        .flatten()
        .filter_map(|entry| {
            entry.path().parent().map(|parent| {
                (
                    entry.path().to_path_buf(),
                    entry.depth(),
                    parent.to_owned(),
                    entry.file_name().to_owned(),
                )
            })
        })
    {
        let curr_folder_addr = *folders
            .entry(dir_path)
            .or_insert(FoldersApi::new(client.clone(), root_dir))
            .address();

        if depth > 0 {
            let parent_folder = folders
                .entry(parent)
                .or_insert(FoldersApi::new(client.clone(), root_dir));
            parent_folder.add_folder(dir_name, curr_folder_addr)?;
        }
    }

    Ok(folders)
}

// Make a single payment for all Folders (Registers) and upload them to the network
async fn pay_and_upload_folders(
    folders: BTreeMap<PathBuf, FoldersApi>,
    verify_store: bool,
    client: &Client,
    root_dir: &Path,
) -> Result<()> {
    // Let's make the storage payment
    let mut wallet_client = WalletClient::new(client.clone(), HotWallet::load_from(root_dir)?);
    let net_addresses = folders.values().map(|folder| folder.as_net_addr());
    let payment_result = wallet_client.pay_for_storage(net_addresses).await?;
    let balance = wallet_client.balance();
    match payment_result
        .storage_cost
        .checked_add(payment_result.royalty_fees)
    {
        Some(cost) => println!(
            "Made payment of {cost} for {} Folders. New balance: {balance}",
            folders.len()
        ),
        None => bail!("Failed to calculate total payment cost"),
    }

    // sync Folders concurrently
    let mut tasks = JoinSet::new();
    for (path, mut folder) in folders {
        let net_addr = folder.as_net_addr();
        let payment_info = wallet_client.get_payment_for_addr(&net_addr)?;

        tasks.spawn(async move {
            match folder.sync(verify_store, Some(payment_info)).await {
                Ok(addr) => println!(
                    "Folder (for {}) synced with the network at: {}",
                    path.display(),
                    addr.to_hex()
                ),
                Err(err) => println!(
                    "Failed to sync Folder (for {}) with the network: {err}",
                    path.display(),
                ),
            }
        });
    }

    while let Some(res) = tasks.join_next().await {
        if let Err(err) = res {
            println!("Failed to sync a Folder with the network: {err:?}");
        }
    }

    Ok(())
}

// Download a Folder from the network and keep track of its subfolders and files
async fn download_folder(
    root_dir: &Path,
    client: &Client,
    target_path: &Path,
    folder_addr: RegisterAddress,
    files_to_download: &mut Vec<(OsString, ChunkAddress, PathBuf)>,
    folders_to_download: &mut Vec<(OsString, RegisterAddress, PathBuf)>,
) -> Result<()> {
    create_dir_all(target_path)?;
    let folders_api = FoldersApi::retrieve(client.clone(), root_dir, folder_addr).await?;

    for (file_name, folder_entry) in folders_api.entries()?.into_iter() {
        match folder_entry {
            FolderEntry::File(file_addr) => {
                files_to_download.push((file_name, file_addr, target_path.to_path_buf()))
            }
            FolderEntry::Folder(subfolder_addr) => {
                folders_to_download.push((
                    file_name.clone(),
                    subfolder_addr,
                    target_path.join(file_name),
                ));
            }
        }
    }

    Ok(())
}
