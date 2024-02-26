// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::files::ChunkManager;

use serde::{Deserialize, Serialize};
use sn_client::{Client, FilesApi, FolderEntry, FoldersApi, Metadata, WalletClient};
use sn_protocol::storage::{Chunk, ChunkAddress, RegisterAddress, RetryStrategy};
use sn_transfers::HotWallet;

use crate::subcommands::files::download::download_file;
use crate::subcommands::files::upload::{
    upload_files_with_iter, FilesUploadOptions, UploadedFile, UPLOADED_FILES,
};
use color_eyre::{
    eyre::{bail, eyre},
    Result,
};
use std::{
    collections::{btree_map::Entry, BTreeMap},
    ffi::OsString,
    fs::{create_dir_all, File},
    io::Write,
    path::{Path, PathBuf},
};
use tokio::task::JoinSet;
use walkdir::{DirEntry, WalkDir};
use xor_name::XorName;

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
    curr_metadata: BTreeMap<PathBuf, MetadataTrackingInfo>,
    root_dir_xorname: RegisterAddress,
}

impl AccountPacket {
    /// Create AccountPacket instance.
    pub fn from_path(client: Client, wallet_dir: &Path, path: &Path) -> Result<Self> {
        let (files_dir, tracking_info_dir, meta_dir) = build_tracking_info_paths(path)?;

        let (curr_metadata, root_dir_xorname) =
            read_folders_metadata_from_disk(&client, &meta_dir)?;

        Ok(Self {
            client,
            wallet_dir: wallet_dir.to_path_buf(),
            files_dir,
            meta_dir,
            tracking_info_dir,
            curr_metadata,
            root_dir_xorname,
        })
    }

    /// Add all files found in the set path to start keeping track of them and changes on them.
    /// Once they have been added, they can be compared against their remote versions on the network
    /// using the `status` method, and/or push all changes to the network with `push` method.
    pub async fn add_all_files(&mut self, options: FilesUploadOptions) -> Result<RegisterAddress> {
        let folders = self.read_files_and_folders_from_disk(options.make_data_public, true)?;

        store_root_folder_tracking_info(&self.meta_dir, self.root_dir_xorname)?;

        println!("Paying for folders hierarchy and uploading...");
        self.pay_and_upload_folders(folders, options).await?;

        Ok(self.root_dir_xorname)
    }

    pub async fn retrieve_folders(
        client: &Client,
        wallet_dir: &Path,
        address: RegisterAddress,
        download_path: &Path,
        batch_size: usize,
        retry_strategy: RetryStrategy,
    ) -> Result<Self> {
        create_dir_all(download_path)?;
        let folder_name: OsString = download_path.file_name().unwrap_or_default().into();
        let (_, tracking_info_dir, meta_dir) = build_tracking_info_paths(download_path)?;

        store_root_folder_tracking_info(&meta_dir, address)?;

        let mut files_to_download = vec![];
        let mut folders_to_download = vec![(folder_name, address, download_path.to_path_buf())];

        while let Some((name, folder_addr, target_path)) = folders_to_download.pop() {
            println!(
                "Downloading Folder {name:?} from {}",
                hex::encode(folder_addr.xorname())
            );

            download_folder_from_network(
                client,
                wallet_dir,
                &target_path,
                folder_addr,
                &mut files_to_download,
                &mut folders_to_download,
            )
            .await?;
        }

        let files_api: FilesApi = FilesApi::new(client.clone(), download_path.to_path_buf());
        let uploaded_files_path = tracking_info_dir.join(UPLOADED_FILES);
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

        let mut acc_packet = Self::from_path(client.clone(), wallet_dir, download_path)?;

        // TODO: generate and store the tracking info without chunking by just reading the generated Folders hierarchy
        acc_packet.read_files_and_folders_from_disk(false, true)?;

        Ok(acc_packet)
    }

    /// Generate a report with differences found in local files/folders in comparison with their versions stored on the network.
    /// TODO: make it receive &self immutable
    pub async fn status(&mut self) -> Result<()> {
        let mut folders = self.read_files_and_folders_from_disk(false, false)?;

        let mut num_of_diffs = 0;
        println!("Looking for local changes made to files/folders...");

        // let's first compare from current files/folders read from disk, with the previous versions of them
        for (folder_path, folder) in folders.iter_mut() {
            for (meta_xorname, metadata) in folder.entries().await? {
                let file_path = folder_path.join(&metadata.name);

                // try to find the tracking info of the file/folder by its name
                match self.get_tracking_info(&file_path) {
                    Ok(Some(tracking_info)) => {
                        match (&tracking_info.metadata.content, metadata.content) {
                            (FolderEntry::File(_), FolderEntry::File(_)) => {
                                if tracking_info.meta_xorname != meta_xorname {
                                    num_of_diffs += 1;
                                    println!("- File content changed: {file_path:?}",);
                                }
                            }
                            (FolderEntry::Folder(_), FolderEntry::Folder(_)) => {}
                            (FolderEntry::Folder(_), FolderEntry::File(_)) => {
                                num_of_diffs += 1;
                                println!(
                                    "- New file found where there used to be a folder: {file_path:?}"
                                );
                            }
                            (FolderEntry::File(_), FolderEntry::Folder(_)) => {
                                num_of_diffs += 1;
                                println!(
                                    "- New folder found where there used to be a file: {file_path:?}"
                                );
                            }
                        }
                    }
                    Ok(None) | Err(_) => {
                        num_of_diffs += 1;
                        match metadata.content {
                            FolderEntry::File(_) => println!("- New file: {file_path:?}"),
                            FolderEntry::Folder(_) => println!("- New folder: {file_path:?}"),
                        }
                    }
                }
            }
        }

        // now let's check if any file/folder was removed from disk
        for (item_path, tracking_info) in self.curr_metadata.iter() {
            let abs_path = self.files_dir.join(item_path);
            match tracking_info.metadata.content {
                FolderEntry::Folder(_) => match folders.get(&abs_path) {
                    Some(_) => {}
                    None => {
                        num_of_diffs += 1;
                        println!("- Folder removed: {abs_path:?}");
                    }
                },
                FolderEntry::File(_) => {
                    match abs_path.parent().and_then(|parent| folders.get_mut(parent)) {
                        Some(folder) => {
                            if folder
                                .entries()
                                .await?
                                .iter()
                                .all(|(_, metadata)| metadata.name != tracking_info.metadata.name)
                            {
                                num_of_diffs += 1;
                                println!("- File removed: {abs_path:?}");
                            }
                        }
                        None => {
                            num_of_diffs += 1;
                            println!("- File removed along with its folder: {abs_path:?}");
                        }
                    }
                }
            }
        }

        println!("Changes found to local files/folders: {num_of_diffs}");
        Ok(())
    }

    // Private helpers

    fn get_relative_path(&self, path: &Path) -> Result<PathBuf> {
        let relative_path = path
            .to_path_buf()
            .canonicalize()?
            .strip_prefix(&self.files_dir)?
            .to_path_buf();
        Ok(relative_path)
    }

    // Store tracking info in a file to keep track of any changes made to the source file/folder
    fn store_tracking_info(
        &self,
        src_path: &Path,
        metadata: Metadata,
        meta_xorname: XorName,
    ) -> Result<()> {
        let metadata_file_path = self.meta_dir.join(hex::encode(meta_xorname));
        let mut meta_file = File::create(metadata_file_path)?;

        let file_path = self.get_relative_path(src_path)?;
        let tracking_info = MetadataTrackingInfo {
            file_path,
            meta_xorname,
            metadata,
        };

        meta_file.write_all(&rmp_serde::to_vec(&tracking_info)?)?;

        Ok(())
    }

    fn read_files_and_folders_from_disk(
        &mut self, // TODO: make it immutable
        _make_data_public: bool,
        store_tracking_info: bool,
    ) -> Result<BTreeMap<PathBuf, FoldersApi>> {
        // TODO: we need to encrypt the head data-map and metadata if make_data_public is false,
        // as well as store both of them all within the metadata chunk, rather than as a separate chunk.
        let include_data_map = true;

        let mut chunk_manager = ChunkManager::new(&self.tracking_info_dir);
        // we never used the local cache so we can realise of any changes made to files content.
        chunk_manager.chunk_with_iter(self.iter_only_files(), false, include_data_map)?;

        let mut folders = self.read_folders_hierarchy_from_disk(store_tracking_info)?;

        // add chunked files to the corresponding Folders
        for chunked_file in chunk_manager.iter_chunked_files() {
            if let Some(parent) = chunked_file.file_path.parent() {
                if let Entry::Occupied(mut entry) = folders.entry(parent.to_path_buf()) {
                    let folder_path = entry.key().clone();
                    if folder_path == self.files_dir {
                        self.root_dir_xorname = *entry.get().address();
                    }

                    let (metadata, meta_xorname) = entry.get_mut().add_file(
                        chunked_file.file_name.clone(),
                        chunked_file.head_chunk_address,
                    )?;

                    if store_tracking_info {
                        let file_path = folder_path.join(&metadata.name);
                        self.store_tracking_info(&file_path, metadata, meta_xorname)?;
                    }
                }
            }
        }

        Ok(folders)
    }

    // Build Folders hierarchy from the set files dir
    fn read_folders_hierarchy_from_disk(
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
                .or_insert(self.find_folder_in_tracking_info(&dir_path)?)
                .address();

            if depth > 0 {
                let parent_folder = folders
                    .entry(parent.clone())
                    .or_insert(self.find_folder_in_tracking_info(&parent)?);
                let (metadata, meta_xorname) =
                    parent_folder.add_folder(dir_name, curr_folder_addr)?;

                if store_tracking_info {
                    self.store_tracking_info(&dir_path, metadata, meta_xorname)?;
                }
            }
        }

        Ok(folders)
    }

    fn get_tracking_info(&self, path: &Path) -> Result<Option<&MetadataTrackingInfo>> {
        let path = self.get_relative_path(path)?;
        Ok(self.curr_metadata.get(&path))
    }

    fn find_folder_in_tracking_info(&self, path: &Path) -> Result<FoldersApi> {
        let address = self.get_tracking_info(path)?.and_then(|tracking_info| {
            match tracking_info.metadata.content {
                FolderEntry::Folder(addr) => Some(addr.xorname()),
                FolderEntry::File(_) => None,
            }
        });

        let folder_api = FoldersApi::new(self.client.clone(), &self.wallet_dir, address)?;
        Ok(folder_api)
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
}

fn build_tracking_info_paths(path: &Path) -> Result<(PathBuf, PathBuf, PathBuf)> {
    let files_dir = path.to_path_buf().canonicalize()?;
    let tracking_info_dir = files_dir.join(SAFE_TRACKING_CHANGES_DIR);
    let meta_dir = tracking_info_dir.join(METADATA_CACHE_DIR);
    create_dir_all(&meta_dir)
        .map_err(|err| eyre!("The path provided needs to be a directory: {err}"))?;

    Ok((files_dir, tracking_info_dir, meta_dir))
}

// Download a Folder from the network and keep track of its subfolders and files
async fn download_folder_from_network(
    client: &Client,
    wallet_dir: &Path,
    target_path: &Path,
    folder_addr: RegisterAddress,
    files_to_download: &mut Vec<(OsString, ChunkAddress, PathBuf)>,
    folders_to_download: &mut Vec<(OsString, RegisterAddress, PathBuf)>,
) -> Result<()> {
    create_dir_all(target_path)?;
    let mut folders_api = FoldersApi::retrieve(client.clone(), wallet_dir, folder_addr).await?;
    for (_, Metadata { name, content }) in folders_api.entries().await?.into_iter() {
        match content {
            FolderEntry::File(file_addr) => {
                files_to_download.push((name.clone().into(), file_addr, target_path.to_path_buf()))
            }
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

fn read_folders_metadata_from_disk(
    client: &Client,
    meta_dir: &Path,
) -> Result<(BTreeMap<PathBuf, MetadataTrackingInfo>, RegisterAddress)> {
    let root_folder_xorname = read_root_folder_xorname(meta_dir).unwrap_or_else(|_| {
        let mut rng = rand::thread_rng();
        RegisterAddress::new(XorName::random(&mut rng), client.signer_pk())
    });

    let mut curr_metadata = BTreeMap::new();
    for entry in WalkDir::new(meta_dir)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file() && e.file_name() != ROOT_FOLDER_METADATA_FILENAME)
    {
        let path = entry.path();
        let bytes = std::fs::read(path)
            .map_err(|err| eyre!("Error while reading the tracking info from {path:?}: {err}"))?;
        let tracking_info: MetadataTrackingInfo = rmp_serde::from_slice(&bytes)
            .map_err(|err| eyre!("Error while deserializing tracking info from {path:?}: {err}"))?;

        curr_metadata.insert(tracking_info.file_path.clone(), tracking_info);
    }

    Ok((curr_metadata, root_folder_xorname))
}

// Store tracking info about the root folder in a file to keep track of any changes made
fn store_root_folder_tracking_info(
    meta_dir: &Path,
    root_folder_xorname: RegisterAddress,
) -> Result<()> {
    let path = meta_dir.join(ROOT_FOLDER_METADATA_FILENAME);
    let mut meta_file = File::create(path)?;
    meta_file.write_all(root_folder_xorname.to_hex().as_bytes())?;

    Ok(())
}

fn read_root_folder_xorname(meta_dir: &Path) -> Result<RegisterAddress> {
    let path = meta_dir.join(ROOT_FOLDER_METADATA_FILENAME);
    let bytes = std::fs::read(&path)
        .map_err(|err| eyre!("Error while reading the tracking info from {path:?}: {err}"))?;

    Ok(RegisterAddress::from_hex(&String::from_utf8(bytes)?)?)
}
