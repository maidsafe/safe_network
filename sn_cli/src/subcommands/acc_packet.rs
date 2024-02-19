// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::files::{download_file, ChunkManager};

use serde::{Deserialize, Serialize};
use sn_client::{Client, FilesApi, FolderEntry, FoldersApi, Metadata, WalletClient};
use sn_protocol::storage::{Chunk, ChunkAddress, RegisterAddress, RetryStrategy};
use sn_transfers::HotWallet;

use crate::subcommands::files::upload::{
    upload_files_with_iter, FilesUploadOptions, UploadedFile, UPLOADED_FILES,
};
use color_eyre::{
    eyre::{bail, eyre},
    Result,
};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs::{create_dir_all, File},
    io::Write,
    path::{Path, PathBuf},
};
use tokio::task::JoinSet;
use walkdir::{DirEntry, WalkDir};
use xor_name::{XorName, XOR_NAME_LEN};

// Name of hidden folder where tracking information and metadata is locally kept.
const SAFE_TRACKING_CHANGES_DIR: &str = ".safe";

// Subfolder where files metadata will be cached
const METADATA_CACHE_DIR: &str = "metadata";

// Name of the file where metadata about root folder is locally kept.
const ROOT_FOLDER_METADATA_FILENAME: &str = "root_folder.xorname";

// Information stored locally to keep track of local changes to files/folders.
// TODO: to make file changes discovery more efficient, add more info like file size and last modified timestamp.
#[derive(Debug, Serialize, Deserialize)]
struct MetadataTrackingInfo {
    file_path: PathBuf,
    meta_xorname: XorName,
    metadata: Metadata,
}

pub struct AccountPacket {
    client: Client,
    wallet_dir: PathBuf,
    files_dir: PathBuf,
    meta_dir: PathBuf,
    tracking_info_dir: PathBuf,
}

impl AccountPacket {
    /// Create AccountPacket instance.
    pub fn new(client: Client, wallet_dir: &Path, path: &Path) -> Result<Self> {
        let files_dir = path.to_path_buf().canonicalize()?;
        let tracking_info_dir = files_dir.join(SAFE_TRACKING_CHANGES_DIR);
        let meta_dir = tracking_info_dir.join(METADATA_CACHE_DIR);
        create_dir_all(&meta_dir)
            .map_err(|err| eyre!("The path provided needs to be a directory: {err}"))?;

        Ok(Self {
            client,
            wallet_dir: wallet_dir.to_path_buf(),
            files_dir,
            meta_dir,
            tracking_info_dir,
        })
    }

    /// Add all files found in the set path to start keeping track of them and changes on them.
    /// Once they have been added, they can be compared against their remote versions on the network
    /// using the `status` method, and/or push all changes to the network with `sync` method.
    pub async fn add_all_files(&mut self, options: FilesUploadOptions) -> Result<RegisterAddress> {
        let mut chunk_manager = ChunkManager::new(&self.tracking_info_dir);
        chunk_manager.chunk_with_iter(self.iter_only_files(), true, options.make_data_public)?;

        let mut folders = self.build_folders_hierarchy(true)?;

        // add chunked files to the corresponding Folders
        for chunked_file in chunk_manager.iter_chunked_files() {
            if let Some(parent) = chunked_file.file_path.parent() {
                if let Some(folder) = folders.get_mut(parent) {
                    let (metadata, meta_xorname) = folder.add_file(
                        chunked_file.file_name.clone(),
                        chunked_file.head_chunk_address,
                    )?;

                    self.store_tracking_info(&chunked_file.file_path, metadata, meta_xorname)?;
                }
            }
        }

        println!("Paying for folders hierarchy and uploading...");
        let root_dir_address = folders
            .get(&self.files_dir)
            .map(|folder| *folder.address())
            .ok_or(eyre!("Failed to obtain main Folder network address"))?;
        self.store_root_folder_tracking_info(root_dir_address.xorname())?;

        self.pay_and_upload_folders(folders, options).await?;

        Ok(root_dir_address)
    }

    pub async fn download_folders(
        &self,
        address: RegisterAddress,
        folder_name: OsString,
        download_path: &Path,
        batch_size: usize,
        retry_strategy: RetryStrategy,
    ) -> Result<()> {
        let mut files_to_download = vec![];
        let mut folders_to_download = vec![(folder_name, address, download_path.to_path_buf())];

        while let Some((name, folder_addr, target_path)) = folders_to_download.pop() {
            println!(
                "Downloading Folder {name:?} from {}",
                hex::encode(folder_addr.xorname())
            );

            self.download_folder(
                &target_path,
                folder_addr,
                &mut files_to_download,
                &mut folders_to_download,
            )
            .await?;
        }

        let files_api: FilesApi = FilesApi::new(self.client.clone(), download_path.to_path_buf());
        let uploaded_files_path = self.tracking_info_dir.join(UPLOADED_FILES);
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

        Ok(())
    }

    /// Generate a report with differences found in local files/folders in comparison with their versions stored on the network.
    pub fn status(&mut self) -> Result<()> {
        let root_folder_xorname = self.read_root_folder_xorname()?;
        let mut curr_folders = BTreeMap::<PathBuf, FoldersApi>::new();
        curr_folders.insert(
            PathBuf::new(),
            FoldersApi::with_xorname(self.client.clone(), &self.wallet_dir, root_folder_xorname)?,
        );

        for entry in WalkDir::new(&self.meta_dir)
            .into_iter()
            .flatten()
            .filter(|e| e.file_type().is_file() && e.file_name() != ROOT_FOLDER_METADATA_FILENAME)
        {
            let path = entry.path();
            let bytes = std::fs::read(path).map_err(|err| {
                eyre!("Error while reading the tracking info from {path:?}: {err}")
            })?;
            let tracking_info: MetadataTrackingInfo =
                rmp_serde::from_slice(&bytes).map_err(|err| {
                    eyre!("Error while deserializing tracking info from {path:?}: {err}")
                })?;

            println!(">> INFO {tracking_info:?}");

            /*let curr_folder_addr = *curr_folders
            .entry(tracking_info.clone())
            .or_insert(FoldersApi::with_xorname(
                self.client.clone(),
                &self.wallet_dir,
                tracking_info.meta_xorname,
            )?)
            .address();*/
        }

        /*
        let make_public = false;
        let mut chunk_manager = ChunkManager::new(&self.tracking_info_dir);
        chunk_manager.chunk_with_iter(self.iter_only_files(), true, make_public)?;

        let mut folders = self.build_folders_hierarchy(false)?;

        // add chunked files to the corresponding Folders
        for chunked_file in chunk_manager.iter_chunked_files() {
            if let Some(parent) = chunked_file.file_path.parent() {
                if let Some(folder) = folders.get_mut(parent) {
                    println!(
                        ">> FILE {:?} -> {}",
                        chunked_file.file_path,
                        hex::encode(chunked_file.head_chunk_address.xorname())
                    );
                    let (_metadata, meta_xorname) = folder.add_file(
                        chunked_file.file_name.clone(),
                        chunked_file.head_chunk_address,
                    )?;

                    let metadata_file = self.meta_dir.join(hex::encode(meta_xorname));
                    if metadata_file.exists() {
                        println!(
                            "META FOUND for {:?} >> {metadata_file:?}",
                            chunked_file.file_name
                        );
                    } else {
                        println!(
                            "NOT FOUND for {:?} >> {metadata_file:?}",
                            chunked_file.file_name
                        );
                    }
                }
            }
        }
        */

        Ok(())
    }

    // Private helpers

    // Store tracking info in a file to keep track of any changes made to the source file/folder
    fn store_tracking_info(
        &self,
        src_path: &Path,
        metadata: Metadata,
        meta_xorname: XorName,
    ) -> Result<()> {
        let metadata_file_path = self.meta_dir.join(hex::encode(meta_xorname));
        let mut meta_file = File::create(metadata_file_path)?;

        let file_path = src_path
            .to_path_buf()
            .canonicalize()?
            .strip_prefix(&self.files_dir)?
            .to_path_buf();
        let tracking_info = MetadataTrackingInfo {
            file_path,
            meta_xorname,
            metadata,
        };

        meta_file.write_all(&rmp_serde::to_vec(&tracking_info)?)?;

        Ok(())
    }

    // Store tracking info about the root folder in a file to keep track of any changes made
    fn store_root_folder_tracking_info(&self, root_folder_xorname: XorName) -> Result<()> {
        let path = self.meta_dir.join(ROOT_FOLDER_METADATA_FILENAME);
        let mut meta_file = File::create(path)?;
        meta_file.write_all(hex::encode(root_folder_xorname).as_bytes())?;

        Ok(())
    }

    fn read_root_folder_xorname(&self) -> Result<XorName> {
        let path = self.meta_dir.join(ROOT_FOLDER_METADATA_FILENAME);
        let bytes = std::fs::read(&path)
            .map_err(|err| eyre!("Error while reading the tracking info from {path:?}: {err}"))?;

        let mut xorname = [0; XOR_NAME_LEN];
        xorname.copy_from_slice(&hex::decode(bytes)?);
        Ok(XorName(xorname))
    }

    // Build Folders hierarchy from the set files dir
    fn build_folders_hierarchy(
        &self,
        store_tracking_info: bool,
    ) -> Result<BTreeMap<PathBuf, FoldersApi>> {
        let mut folders = BTreeMap::new();
        for (dir_path, depth, parent, dir_name) in self.iter_only_dirs().filter_map(|entry| {
            entry.path().parent().map(|parent| {
                (
                    entry.path().to_path_buf(),
                    entry.depth(),
                    parent.to_owned(),
                    entry.file_name().to_owned(),
                )
            })
        }) {
            let curr_folder_addr = *folders
                .entry(dir_path.clone())
                .or_insert(FoldersApi::new(self.client.clone(), &self.wallet_dir)?)
                .address();

            if depth > 0 {
                let parent_folder = folders
                    .entry(parent)
                    .or_insert(FoldersApi::new(self.client.clone(), &self.wallet_dir)?);
                let (metadata, meta_xorname) =
                    parent_folder.add_folder(dir_name, curr_folder_addr)?;

                if store_tracking_info {
                    self.store_tracking_info(&dir_path, metadata, meta_xorname)?;
                }
            }
        }

        Ok(folders)
    }

    // Creates an iterator over the user's dirs names, excluding the '.safe' tracking dir
    fn iter_only_dirs(&self) -> impl Iterator<Item = DirEntry> {
        WalkDir::new(&self.files_dir)
            .into_iter()
            .filter_entry(|e| e.file_type().is_dir() && e.file_name() != SAFE_TRACKING_CHANGES_DIR)
            .flatten()
    }

    // Creates an iterator over the user's file, excluding the tracking files under '.safe' dir
    fn iter_only_files(&self) -> impl Iterator<Item = DirEntry> {
        WalkDir::new(&self.files_dir)
            .into_iter()
            .filter_entry(|e| e.file_type().is_file() || e.file_name() != SAFE_TRACKING_CHANGES_DIR)
            .flatten()
            .filter(|e| e.file_type().is_file())
    }

    // Make a single payment for all Folders (Registers) and upload them to the network
    async fn pay_and_upload_folders(
        &self,
        folders: BTreeMap<PathBuf, FoldersApi>,
        options: FilesUploadOptions,
    ) -> Result<()> {
        upload_files_with_iter(
            self.iter_only_files(),
            self.files_dir.clone(),
            &self.client,
            self.wallet_dir.clone(),
            self.tracking_info_dir.clone(),
            options.clone(),
        )
        .await?;

        // Let's make the storage payment for Folders
        let mut wallet_client =
            WalletClient::new(self.client.clone(), HotWallet::load_from(&self.wallet_dir)?);
        let mut net_addresses = vec![];
        folders.values().for_each(|folder| {
            net_addresses.extend(folder.meta_addrs_to_pay());
            net_addresses.push(folder.as_net_addr());
        });

        let payment_result = wallet_client
            .pay_for_storage(net_addresses.into_iter())
            .await?;
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
            tasks.spawn(async move {
                match folder
                    .sync(options.verify_store, Some(options.retry_strategy))
                    .await
                {
                    Ok(()) => println!(
                        "Folder (for {path:?}) synced with the network at: {}",
                        folder.address().to_hex()
                    ),
                    Err(err) => {
                        println!("Failed to sync Folder (for {path:?}) with the network: {err}",)
                    }
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
        &self,
        target_path: &Path,
        folder_addr: RegisterAddress,
        files_to_download: &mut Vec<(OsString, ChunkAddress, PathBuf)>,
        folders_to_download: &mut Vec<(OsString, RegisterAddress, PathBuf)>,
    ) -> Result<()> {
        create_dir_all(target_path)?;
        let mut folders_api =
            FoldersApi::retrieve(self.client.clone(), &self.wallet_dir, folder_addr).await?;
        for Metadata { name, content } in folders_api.entries().await?.into_iter() {
            match content {
                FolderEntry::File(file_addr) => files_to_download.push((
                    name.clone().into(),
                    file_addr,
                    target_path.to_path_buf(),
                )),
                FolderEntry::Folder(subfolder_addr) => {
                    folders_to_download.push((
                        name.clone().into(),
                        subfolder_addr,
                        target_path.join(name),
                    ));
                }
            }
        }

        Ok(())
    }
}
