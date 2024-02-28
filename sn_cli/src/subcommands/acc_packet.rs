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
use sn_protocol::storage::{Chunk, RegisterAddress, RetryStrategy};
use sn_registers::EntryHash;
use sn_transfers::HotWallet;

use crate::subcommands::files::download::download_file;
use crate::subcommands::files::iterative_uploader::IterativeUploader;
use crate::subcommands::files::upload::FilesUploadOptions;
use color_eyre::{
    eyre::{bail, eyre},
    Result,
};
use std::{
    collections::{btree_map::Entry, BTreeMap},
    ffi::OsString,
    fs::{create_dir_all, remove_file, File},
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
const ROOT_FOLDER_METADATA_FILENAME: &str = "root_folder.addr";

type Folders = BTreeMap<PathBuf, (FoldersApi, DetectedChange)>;

#[derive(Default)]
struct ChangesToApply {
    folders: BTreeMap<PathBuf, (FoldersApi, DetectedChange)>,
    mutations: Vec<Change>,
}

#[derive(Debug)]
enum Change {
    NewFile(MetadataTrackingInfo),
    FileRemoved((PathBuf, XorName)),
    FileContentChanged((XorName, MetadataTrackingInfo)),
    NewFolder(MetadataTrackingInfo),
    FolderRemoved((PathBuf, XorName)),
}

#[derive(Clone, Debug, PartialEq)]
enum DetectedChange {
    None,
    NewFolder,
    NewEntries,
}

// Information stored locally to keep track of local changes to files/folders.
// TODO: to make file changes discovery more efficient, add more info like file size and last modified timestamp.
#[derive(Debug, Serialize, Deserialize)]
struct MetadataTrackingInfo {
    file_path: PathBuf,
    meta_xorname: XorName,
    metadata: Metadata,
    entry_hash: EntryHash,
}

pub struct AccountPacket {
    client: Client,
    wallet_dir: PathBuf,
    files_dir: PathBuf,
    meta_dir: PathBuf,
    tracking_info_dir: PathBuf,
    curr_metadata: BTreeMap<PathBuf, MetadataTrackingInfo>,
    root_folder_addr: RegisterAddress,
}

impl AccountPacket {
    /// Create AccountPacket instance.
    pub fn from_path(client: Client, wallet_dir: &Path, path: &Path) -> Result<Self> {
        let (files_dir, tracking_info_dir, meta_dir) = build_tracking_info_paths(path)?;

        let (curr_metadata, root_folder_addr) = read_tracking_info_from_disk(&client, &meta_dir)?;

        Ok(Self {
            client,
            wallet_dir: wallet_dir.to_path_buf(),
            files_dir,
            meta_dir,
            tracking_info_dir,
            curr_metadata,
            root_folder_addr,
        })
    }

    /// Add all files found in the set path, uploading them to the network, and start keeping track of them.
    pub async fn add_all_files(&self, options: FilesUploadOptions) -> Result<RegisterAddress> {
        let folders = self.read_files_and_folders_from_disk(options.make_data_public, true)?;

        println!("Paying for folders hierarchy and uploading...");
        self.pay_and_upload_folders(folders, options).await?;

        Ok(self.root_folder_addr)
    }

    /// Retrieve and store entire Folders hierarchy from the network, generating tracking info.
    pub async fn retrieve_folders(
        client: &Client,
        wallet_dir: &Path,
        address: RegisterAddress,
        download_path: &Path,
        batch_size: usize,
        retry_strategy: RetryStrategy,
    ) -> Result<Self> {
        create_dir_all(download_path)?;
        let (files_dir, tracking_info_dir, meta_dir) = build_tracking_info_paths(download_path)?;

        if let Ok(addr) = read_root_folder_addr(&meta_dir) {
            // bail out if there is already a root folder address different from the passed in
            if addr != address {
                bail!(
                    "The download path is already tracking another Folder with address: {}",
                    addr.to_hex()
                );
            }

            // TODO: merge what we'll retrieve from network into what exists locally
        } else {
            store_root_folder_tracking_info(&meta_dir, address)?;
        }

        let mut acc_packet = Self {
            client: client.clone(),
            wallet_dir: wallet_dir.to_path_buf(),
            files_dir,
            meta_dir,
            tracking_info_dir,
            curr_metadata: BTreeMap::default(),
            root_folder_addr: address,
        };

        let mut files_to_download = vec![];
        let folder_name: OsString = download_path.file_name().unwrap_or_default().into();
        let mut folders_to_download = vec![(folder_name, address, download_path.to_path_buf())];

        while let Some((name, folder_addr, target_path)) = folders_to_download.pop() {
            println!(
                "Downloading Folder {name:?} from {}",
                hex::encode(folder_addr.xorname())
            );

            acc_packet
                .download_folder_from_network(
                    &target_path,
                    folder_addr,
                    &mut files_to_download,
                    &mut folders_to_download,
                )
                .await?;
        }

        let files_api: FilesApi = FilesApi::new(client.clone(), download_path.to_path_buf());
        for (file_name, data_map_chunk, path) in files_to_download {
            download_file(
                files_api.clone(),
                *data_map_chunk.name(),
                (file_name, Some(data_map_chunk)),
                &path,
                false,
                batch_size,
                retry_strategy,
            )
            .await;
        }

        let (curr_metadata, _) = read_tracking_info_from_disk(client, &acc_packet.meta_dir)?;
        acc_packet.curr_metadata = curr_metadata;

        Ok(acc_packet)
    }

    /// Generate a report with differences found in local files/folders in comparison with their versions stored on the network.
    pub fn status(&self) -> Result<()> {
        println!("Looking for local changes made to files/folders...");
        let changes = self.scan_files_and_folders_for_changes(false, false)?;

        println!("Local changes made to files/folders:");
        for mutation in &changes.mutations {
            match mutation {
                Change::NewFile(tracking_info) => {
                    println!("New file: {:?}", tracking_info.file_path)
                }
                Change::FileRemoved((path, _)) => println!("File removed: {path:?}"),
                Change::FileContentChanged((_, tracking_info)) => {
                    println!("File content changed: {:?}", tracking_info.file_path)
                }
                Change::NewFolder(tracking_info) => {
                    println!("New folder: {:?}", tracking_info.file_path)
                }
                Change::FolderRemoved((path, _)) => println!("Folder removed: {path:?}"),
            }
        }

        println!(
            "Changes found to local files/folders: {}",
            changes.mutations.len()
        );
        Ok(())
    }

    /// Sync local changes made to files and folder with their version on the network,
    /// both pulling changes form the network, and pushing changes made locally to the network.
    pub async fn sync(&self) -> Result<()> {
        let mut changes = self.scan_files_and_folders_for_changes(false, false)?;

        let folders = &mut changes.folders;
        for (folder_path, (_folder, detected_change)) in folders.iter() {
            println!(">> {folder_path:?} {detected_change:?}");
        }

        // Let's make the storage payment for Folders
        let mut wallet_client =
            WalletClient::new(self.client.clone(), HotWallet::load_from(&self.wallet_dir)?);
        let mut net_addresses = vec![];
        folders.values().for_each(|(folder, detected_change)| {
            if detected_change == &DetectedChange::None {
                return;
            } else if detected_change == &DetectedChange::NewFolder {
                net_addresses.push(folder.as_net_addr());
            }
            net_addresses.extend(folder.meta_addrs_to_pay());
        });

        let _ = wallet_client
            .pay_for_storage(net_addresses.into_iter())
            .await?;

        for (folder_path, (folder, detected_change)) in folders.iter_mut() {
            println!("\n>> {folder_path:?} {detected_change:?}");
            if detected_change == &DetectedChange::None {
                continue;
            }

            match folder.sync(true, None).await {
                Ok(()) => println!(
                    "Folder (for {folder_path:?}) synced with the network at: {}",
                    folder.address().to_hex()
                ),
                Err(err) => {
                    println!("Failed to sync Folder (for {folder_path:?}) with the network: {err}",)
                }
            }
        }

        // store/remove tracking info locally
        for mutation in changes.mutations {
            match mutation {
                Change::NewFile(tracking_info) | Change::NewFolder(tracking_info) => {
                    println!("==> STORE TRACKING INFO: {tracking_info:?}");
                    self.store_tracking_info(tracking_info)?;
                }
                Change::FileRemoved((path, meta_xorname))
                | Change::FolderRemoved((path, meta_xorname)) => {
                    let metadata_file_path = self.meta_dir.join(hex::encode(meta_xorname));
                    println!("==> TRACKINFO TO REMOVE for {path:?} ==> {metadata_file_path:?}");
                    if let Err(err) = remove_file(&metadata_file_path) {
                        println!(
                            "Failed to remove tracking info file {metadata_file_path:?}: {err}"
                        );
                    }
                }
                Change::FileContentChanged((meta_xorname, tracking_info)) => {
                    println!("==> STORE TRACKING INFO: {tracking_info:?}");
                    self.store_tracking_info(tracking_info)?;

                    let metadata_file_path = self.meta_dir.join(hex::encode(meta_xorname));
                    println!("==> TRACKINFO TO REMOVE for {metadata_file_path:?} ==> {metadata_file_path:?}");
                    if let Err(err) = remove_file(&metadata_file_path) {
                        println!(
                            "Failed to remove tracking info file {metadata_file_path:?}: {err}"
                        );
                    }
                }
            }
        }

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
        MetadataTrackingInfo {
            file_path,
            meta_xorname,
            metadata,
            entry_hash,
        }: MetadataTrackingInfo,
    ) -> Result<()> {
        let metadata_file_path = self.meta_dir.join(hex::encode(meta_xorname));
        let mut meta_file = File::create(metadata_file_path)?;

        let tracking_info = MetadataTrackingInfo {
            // we store the relative path so the root folder can be moved if desired by the user
            file_path: self.get_relative_path(&file_path)?,
            meta_xorname,
            metadata,
            entry_hash,
        };

        meta_file.write_all(&rmp_serde::to_vec(&tracking_info)?)?;

        Ok(())
    }

    // TODO: remove this function
    fn read_files_and_folders_from_disk(
        &self,
        _make_data_public: bool,
        store_tracking_info: bool,
    ) -> Result<Folders> {
        let mut chunk_manager = ChunkManager::new(&self.tracking_info_dir);
        // we never used the local cache so we can realise of any changes made to files content.
        chunk_manager.chunk_with_iter(self.iter_only_files(), false, false)?;

        let changes = self.read_folders_hierarchy_from_disk(store_tracking_info)?;
        if store_tracking_info {
            store_root_folder_tracking_info(&self.meta_dir, self.root_folder_addr)?;
        }

        // add chunked files to the corresponding Folders
        let mut folders = changes.folders;
        for chunked_file in chunk_manager.iter_chunked_files() {
            if let Some(Entry::Occupied(mut parent_folder)) = chunked_file
                .file_path
                .parent()
                .map(|parent| folders.entry(parent.to_path_buf()))
            {
                // TODO: we need to encrypt the data-map and metadata if make_data_public is false.
                let (entry_hash, meta_xorname, metadata) = parent_folder.get_mut().0.add_file(
                    chunked_file.file_name.clone(),
                    chunked_file.data_map.clone(),
                )?;

                if store_tracking_info {
                    let file_path = parent_folder.key().join(&metadata.name);
                    self.store_tracking_info(MetadataTrackingInfo {
                        file_path,
                        meta_xorname,
                        metadata,
                        entry_hash,
                    })?;
                }
            }
        }

        Ok(folders)
    }

    // Scan existing files and folders on disk, generating a report of all the detected
    // changes based on the tracking info kept locally.
    fn scan_files_and_folders_for_changes(
        &self,
        _make_data_public: bool,
        store_tracking_info: bool,
    ) -> Result<ChangesToApply> {
        // we don't use the local cache in order to realise of any changes made to files content.
        let mut chunk_manager = ChunkManager::new(&self.tracking_info_dir);
        chunk_manager.chunk_with_iter(self.iter_only_files(), false, false)?;

        let mut changes = self.read_folders_hierarchy_from_disk(store_tracking_info)?;
        if store_tracking_info {
            store_root_folder_tracking_info(&self.meta_dir, self.root_folder_addr)?;
        }

        // add chunked files to the corresponding Folders
        let folders = &mut changes.folders;
        for chunked_file in chunk_manager.iter_chunked_files() {
            let file_path = &chunked_file.file_path;
            if let Some(Entry::Occupied(mut parent_folder)) = file_path
                .parent()
                .map(|parent| folders.entry(parent.to_path_buf()))
            {
                // try to find the tracking info of the file/folder by its name
                match self.get_tracking_info(file_path) {
                    Ok(Some(tracking_info)) => match &tracking_info.metadata.content {
                        FolderEntry::File(chunk) => {
                            if chunk.address() != &chunked_file.head_chunk_address {
                                if parent_folder.get().1 == DetectedChange::None {
                                    parent_folder.get_mut().1 = DetectedChange::NewEntries;
                                }

                                // TODO: we need to encrypt the data-map and metadata if make_data_public is false.
                                let (entry_hash, meta_xorname, metadata) =
                                    parent_folder.get_mut().0.replace_file(
                                        tracking_info.entry_hash,
                                        chunked_file.file_name.clone(),
                                        chunked_file.data_map.clone(),
                                    )?;

                                changes.mutations.push(Change::FileContentChanged((
                                    tracking_info.meta_xorname,
                                    MetadataTrackingInfo {
                                        file_path: file_path.to_path_buf(),
                                        meta_xorname,
                                        metadata,
                                        entry_hash,
                                    },
                                )));
                            }
                        }
                        FolderEntry::Folder(_) => {
                            // New file found where there used to be a folder
                            // TODO: we need to encrypt the data-map and metadata if make_data_public is false.
                            let (entry_hash, meta_xorname, metadata) =
                                parent_folder.get_mut().0.replace_file(
                                    tracking_info.entry_hash,
                                    chunked_file.file_name.clone(),
                                    chunked_file.data_map.clone(),
                                )?;
                            changes
                                .mutations
                                .push(Change::NewFile(MetadataTrackingInfo {
                                    file_path: file_path.to_path_buf(),
                                    meta_xorname,
                                    metadata,
                                    entry_hash,
                                }));
                        }
                    },
                    Ok(None) => {
                        if parent_folder.get().1 == DetectedChange::None {
                            parent_folder.get_mut().1 = DetectedChange::NewEntries;
                        }

                        // TODO: we need to encrypt the data-map and metadata if make_data_public is false.
                        let (entry_hash, meta_xorname, metadata) =
                            parent_folder.get_mut().0.add_file(
                                chunked_file.file_name.clone(),
                                chunked_file.data_map.clone(),
                            )?;
                        changes
                            .mutations
                            .push(Change::NewFile(MetadataTrackingInfo {
                                file_path: file_path.to_path_buf(),
                                meta_xorname,
                                metadata,
                                entry_hash,
                            }));
                    }
                    Err(err) => {
                        println!("Skipping file {file_path:?}: {err:?}");
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
                        if let Some((parent_folder, _)) =
                            abs_path.parent().and_then(|p| folders.get_mut(p))
                        {
                            parent_folder.remove_item(tracking_info.entry_hash)?;
                        }
                        changes.mutations.push(Change::FolderRemoved((
                            abs_path,
                            tracking_info.meta_xorname,
                        )));
                    }
                },
                FolderEntry::File(_) => {
                    if chunk_manager
                        .iter_chunked_files()
                        .all(|chunked_file| chunked_file.file_path != abs_path)
                    {
                        if let Some((parent_folder, _)) =
                            abs_path.parent().and_then(|p| folders.get_mut(p))
                        {
                            parent_folder.remove_item(tracking_info.entry_hash)?;
                        }

                        changes
                            .mutations
                            .push(Change::FileRemoved((abs_path, tracking_info.meta_xorname)));
                    }
                }
            }
        }

        Ok(changes)
    }

    // Build Folders hierarchy from the set files dir
    fn read_folders_hierarchy_from_disk(
        &self,
        store_tracking_info: bool,
    ) -> Result<ChangesToApply> {
        let mut changes = ChangesToApply::default();
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
            let (folder, detected_change) = changes
                .folders
                .entry(dir_path.clone())
                .or_insert(self.find_folder_in_tracking_info(&dir_path)?)
                .clone();
            let curr_folder_addr = *folder.address();

            if depth > 0 {
                let (parent_folder, _) = changes
                    .folders
                    .entry(parent.clone())
                    .or_insert(self.find_folder_in_tracking_info(&parent)?);
                let (entry_hash, meta_xorname, metadata) =
                    parent_folder.add_folder(dir_name, curr_folder_addr)?;

                if detected_change == DetectedChange::NewFolder {
                    changes
                        .mutations
                        .push(Change::NewFolder(MetadataTrackingInfo {
                            file_path: dir_path,
                            meta_xorname,
                            metadata,
                            entry_hash,
                        }));
                }
            }
        }

        Ok(changes)
    }

    // Read local tracking info for given file/folder item
    fn get_tracking_info(&self, path: &Path) -> Result<Option<&MetadataTrackingInfo>> {
        let path = self.get_relative_path(path)?;
        Ok(self.curr_metadata.get(&path))
    }

    // Instantiate a FolderApi based on local tracking info for given folder item
    fn find_folder_in_tracking_info(&self, path: &Path) -> Result<(FoldersApi, DetectedChange)> {
        let mut detected_change = DetectedChange::NewFolder;
        let mut address =
            self.get_tracking_info(path)?.and_then(|tracking_info| {
                match tracking_info.metadata.content {
                    FolderEntry::Folder(addr) => {
                        detected_change = DetectedChange::None;
                        Some(addr)
                    }
                    FolderEntry::File(_) => None,
                }
            });

        if address.is_none() && path == self.files_dir {
            address = Some(self.root_folder_addr);
            detected_change = DetectedChange::None;
        }
        let folders_api = FoldersApi::new(self.client.clone(), &self.wallet_dir, address)?;
        Ok((folders_api, detected_change))
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

    // Make a single payment for all Folders (Registers) and metadata chunks, and upload them to the network
    async fn pay_and_upload_folders(
        &self,
        folders: Folders,
        options: FilesUploadOptions,
    ) -> Result<()> {
        let files_api = FilesApi::build(self.client.clone(), self.wallet_dir.clone())?;
        let chunk_manager = ChunkManager::new(&self.tracking_info_dir.clone());

        IterativeUploader::new(chunk_manager, files_api)
            .iterate_upload(
                self.iter_only_files(),
                self.files_dir.clone(),
                &self.client,
                options.clone(),
            )
            .await?;

        // Let's make the storage payment for Folders
        let mut wallet_client =
            WalletClient::new(self.client.clone(), HotWallet::load_from(&self.wallet_dir)?);
        let mut net_addresses = vec![];
        folders.values().for_each(|(folder, _)| {
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
        for (path, (mut folder, _)) in folders {
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

    // Download a Folder from the network and generate tracking info
    async fn download_folder_from_network(
        &self,
        target_path: &Path,
        folder_addr: RegisterAddress,
        files_to_download: &mut Vec<(OsString, Chunk, PathBuf)>,
        folders_to_download: &mut Vec<(OsString, RegisterAddress, PathBuf)>,
    ) -> Result<()> {
        create_dir_all(target_path)?;
        let mut folders_api =
            FoldersApi::retrieve(self.client.clone(), &self.wallet_dir, folder_addr).await?;
        for (entry_hash, meta_xorname, metadata) in folders_api.entries().await?.into_iter() {
            let name = metadata.name.clone();
            let item_path = match &metadata.content {
                FolderEntry::File(data_map_chunk) => {
                    files_to_download.push((
                        name.clone().into(),
                        data_map_chunk.clone(),
                        target_path.to_path_buf(),
                    ));
                    let item_path = target_path.join(name);
                    let _ = File::create(&item_path)?;
                    item_path
                }
                FolderEntry::Folder(subfolder_addr) => {
                    folders_to_download.push((
                        name.clone().into(),
                        *subfolder_addr,
                        target_path.join(name),
                    ));
                    target_path.to_path_buf()
                }
            };

            self.store_tracking_info(MetadataTrackingInfo {
                file_path: item_path,
                meta_xorname,
                metadata,
                entry_hash,
            })?;
        }

        Ok(())
    }
}

// Build absolute paths for the different dirs to be used for locally tracking changes
fn build_tracking_info_paths(path: &Path) -> Result<(PathBuf, PathBuf, PathBuf)> {
    let files_dir = path.to_path_buf().canonicalize()?;
    let tracking_info_dir = files_dir.join(SAFE_TRACKING_CHANGES_DIR);
    let meta_dir = tracking_info_dir.join(METADATA_CACHE_DIR);
    create_dir_all(&meta_dir)
        .map_err(|err| eyre!("The path provided needs to be a directory: {err}"))?;

    Ok((files_dir, tracking_info_dir, meta_dir))
}

fn read_tracking_info_from_disk(
    client: &Client,
    meta_dir: &Path,
) -> Result<(BTreeMap<PathBuf, MetadataTrackingInfo>, RegisterAddress)> {
    let root_folder_addr = read_root_folder_addr(meta_dir).unwrap_or_else(|_| {
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

    Ok((curr_metadata, root_folder_addr))
}

// Store tracking info about the root folder in a file to keep track of any changes made
fn store_root_folder_tracking_info(
    meta_dir: &Path,
    root_folder_addr: RegisterAddress,
) -> Result<()> {
    let path = meta_dir.join(ROOT_FOLDER_METADATA_FILENAME);
    let mut meta_file = File::create(path)?;
    meta_file.write_all(root_folder_addr.to_hex().as_bytes())?;

    Ok(())
}

// Read the tracking info about the root folder
fn read_root_folder_addr(meta_dir: &Path) -> Result<RegisterAddress> {
    let path = meta_dir.join(ROOT_FOLDER_METADATA_FILENAME);
    let bytes = std::fs::read(&path)
        .map_err(|err| eyre!("Error while reading the tracking info from {path:?}: {err}"))?;

    Ok(RegisterAddress::from_hex(&String::from_utf8(bytes)?)?)
}
