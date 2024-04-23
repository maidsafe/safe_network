// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod change_tracking;

use change_tracking::*;

use super::{
    files::{download_file, FilesUploader},
    ChunkManager,
};

use sn_client::{
    protocol::storage::{Chunk, RegisterAddress, RetryStrategy},
    registers::EntryHash,
    transfers::{DerivationIndex, HotWallet, MainSecretKey},
    Client, FilesApi, FolderEntry, FoldersApi, Metadata, UploadCfg, WalletClient,
};

use bls::PublicKey;
use color_eyre::{
    eyre::{bail, eyre},
    Result,
};
use std::{
    collections::{
        btree_map::{Entry, OccupiedEntry},
        BTreeMap,
    },
    ffi::OsString,
    fs::{create_dir_all, remove_dir_all, remove_file, File},
    io::Write,
    path::{Path, PathBuf},
};
use tokio::task::JoinSet;
use tracing::trace;
use walkdir::{DirEntry, WalkDir};
use xor_name::XorName;

/// Derivation index used to obtain the account packet root folder xorname
// TODO: use eip2333 path for deriving keys
const ACC_PACKET_ADDR_DERIVATION_INDEX: DerivationIndex = DerivationIndex([0x0; 32]);

/// Derivation index used to obtain the owner key of the account packet root folder.
/// The derived key pair is used to:
/// - Sign all data operations sent to the network.
/// - Set it as the owner of all Folders (Registers) created on the network.
/// - Encrypt all the Folders entries metadata chunks.
// TODO: use eip2333 path for deriving keys
const ACC_PACKET_OWNER_DERIVATION_INDEX: DerivationIndex = DerivationIndex([0x1; 32]);

/// An `AccountPacket` object allows users to store and manage files, wallets, etc., with the ability
/// and tools necessary to keep an instance tracking a local storage path, as well as keeping it in sync
/// with its remote version stored on the network.
/// A `Client` and a the location for a funded local hot-wallet are required by this object in order to be able to connect
/// to the network, paying for data storage, and upload/retrieve information to/from the network.
///
/// TODO: currently only files and folders are supported, wallets, keys, etc., to be added later.
///
/// TODO: make use of eip2333 paths for deriving keys. Currently keys used for encrypting and signing
/// operations are derived from the root key provided using index derivation.
///
/// The `AccountPacket` keeps a reference to the network address of the root Folder holding the user's
/// files/folder hierarchy. All tracking information is kept under the `.safe` directory on disk, whose
/// content is not uploaded to the network, but only kept locally in order to realise which files/dirs
/// the user has made changes on compared to their last version retrieved from the network.
///
/// A subdirectory called `metadata` is kept under `.safe` directory with the following files:
/// - A file named `root_folder.addr` which contains the network address where the root Folder is stored,
/// which is the one holding the entire hierarchy of user's files/dirs to be kept in sync with local changes
/// made by the user.
/// - For each of the user's files/dirs, a serialised `MetadataTrackingInfo` instance is stored on using the
/// file/dir metadata chunk xorname as filename. The information stored in these files are used to realise
/// if changes were locally made by the user in comparison with the last version of such files/dirs retrieved
/// from the network.
/// Example of files generated within an account-packet to keep track of changes makde to user's files/dirs:
///
/// ./my-acc-packet
/// ├── my_dir_1
/// ├── my_file.txt
/// ├── my_dir_2
/// │   ├── other_dir
/// │   └── my_other_file.txt
/// └── .safe
///     ├── chunk_artifacts
///     │   ├── ...
///     │   ...
///     ├── metadata
///     │   ├── 082cc90c900fa08d36067246a1e6136a828f1aae4926268c4349c200d56e34b9
///     │   ├── 102c5536a10682bc3cdd4a1915fe2ad5e839cb94d0d3f124d0c18aee1d49ce50
///     │   ├── 31824937c47a979df64af591f2e43f76190e65af835c4b338cbe7a7ba3f7d3cb
///     │   ├── 36778e471083140bc111677e2a86e49f4c0c20bc14ff2ad610e22615b72260b8
///     │   ├── 3edd953cc320449e09b69b7b1b909a53874ee477f602f1a807dfd8057378367e
///     │   └── root_folder.addr
///     └── uploaded_files
///         ├── ...
///         ...
///
/// There are other files which are stored under `.safe/chunk_artifacts` and `.safe/uploaded_files` directories
/// which are managed by the `ChunkManager` in order to locally cache chunked files, and a list of files
/// already uploaded to the network, to prevent from chunking and/or uploading the same files again. For more
/// details about these files, please refer to the `ChunkManager` module.
pub struct AccountPacket {
    client: Client,
    wallet_dir: PathBuf,
    files_dir: PathBuf,
    meta_dir: PathBuf,
    tracking_info_dir: PathBuf,
    curr_tracking_info: BTreeMap<PathBuf, MetadataTrackingInfo>,
    root_folder_addr: RegisterAddress,
    root_folder_created: bool,
}

impl AccountPacket {
    /// Initialise directory as a fresh new packet.
    /// All keys used for encrypting the files/folders metadata chunks and signing
    /// operations are derived from the root key provided using index derivation.
    /// The root Folder address and owner are also derived from the root SK.
    /// A password can be optionally provided to encrypt the root SK before storing it on disk.
    pub fn init(
        client: Client,
        wallet_dir: &Path,
        path: &Path,
        root_sk: &MainSecretKey,
        password: Option<&[u8]>,
    ) -> Result<Self> {
        let (_, tracking_info_dir, meta_dir) = build_tracking_info_paths(path)?;

        // If there is already some tracking info we bail out as this is meant ot be a fresh new packet.
        if let Ok((addr, _)) = read_root_folder_addr(&meta_dir) {
            bail!(
                "The local path {path:?} is already being tracked with Folder address: {}",
                addr.to_hex()
            );
        }

        let (client, root_folder_addr) = derive_keys_and_address(client, root_sk);
        store_root_folder_tracking_info(&meta_dir, root_folder_addr, false)?;
        store_root_sk(&tracking_info_dir, root_sk, password)?;
        Self::from_path(client, wallet_dir, path, password)
    }

    /// Create AccountPacket instance from a directory which has been already initialised.
    pub fn from_path(
        client: Client,
        wallet_dir: &Path,
        path: &Path,
        password: Option<&[u8]>,
    ) -> Result<Self> {
        let (files_dir, tracking_info_dir, meta_dir) = build_tracking_info_paths(path)?;
        let root_sk = read_root_sk(&tracking_info_dir, password)?;
        let (client, root_folder_addr) = derive_keys_and_address(client, &root_sk);

        // this will fail if the directory was not previously initialised with 'init'.
        let curr_tracking_info = read_tracking_info_from_disk(&meta_dir)?;
        let (read_folder_addr, root_folder_created) = read_root_folder_addr(&meta_dir)
            .map_err(|_| eyre!("Root Folder address not found, make sure the directory {path:?} is initialised."))?;
        if read_folder_addr != root_folder_addr {
            bail!(
                "The path is already tracking another Folder with address: {}",
                read_folder_addr.to_hex()
            );
        }

        Ok(Self {
            client,
            wallet_dir: wallet_dir.to_path_buf(),
            files_dir,
            meta_dir,
            tracking_info_dir,
            curr_tracking_info,
            root_folder_addr,
            root_folder_created,
        })
    }

    /// Return the address of the root Folder
    pub fn root_folder_addr(&self) -> RegisterAddress {
        self.root_folder_addr
    }

    /// Retrieve and store entire Folders hierarchy from the network, generating tracking info.
    pub async fn retrieve_folders(
        client: &Client,
        wallet_dir: &Path,
        root_sk: &MainSecretKey,
        password: Option<&[u8]>,
        download_path: &Path,
        batch_size: usize,
        retry_strategy: RetryStrategy,
    ) -> Result<Self> {
        create_dir_all(download_path)?;
        let (files_dir, tracking_info_dir, meta_dir) = build_tracking_info_paths(download_path)?;

        let (client, root_folder_addr) = derive_keys_and_address(client.clone(), root_sk);

        if let Ok((addr, _)) = read_root_folder_addr(&meta_dir) {
            // bail out if there is already a root folder address different from the passed in
            if addr == root_folder_addr {
                bail!("The download path is already tracking that Folder, use 'sync' instead.");
            } else {
                bail!(
                    "The download path is already tracking another Folder with address: {}",
                    addr.to_hex()
                );
            }
        } else {
            store_root_folder_tracking_info(&meta_dir, root_folder_addr, true)?;
            store_root_sk(&tracking_info_dir, root_sk, password)?;
        }

        let mut acc_packet = Self {
            client: client.clone(),
            wallet_dir: wallet_dir.to_path_buf(),
            files_dir,
            meta_dir,
            tracking_info_dir,
            curr_tracking_info: BTreeMap::default(),
            root_folder_addr,
            root_folder_created: true,
        };

        let folder_name: OsString = download_path.file_name().unwrap_or_default().into();
        let folders_api =
            FoldersApi::retrieve(client.clone(), wallet_dir, root_folder_addr).await?;
        let folders_to_download = vec![(folder_name, folders_api, download_path.to_path_buf())];

        let _ = acc_packet
            .download_folders_and_files(folders_to_download, batch_size, retry_strategy)
            .await?;

        acc_packet.curr_tracking_info = read_tracking_info_from_disk(&acc_packet.meta_dir)?;

        Ok(acc_packet)
    }

    /// Generate a report with differences found in local files/folders in comparison with their versions stored on the network.
    pub fn status(&self) -> Result<()> {
        println!("Looking for local changes made to files/folders compared to version on network at: {} ...", self.root_folder_addr().to_hex());
        let changes = self.scan_files_and_folders_for_changes(false)?;

        if changes.mutations.is_empty() {
            println!("No local changes made to files/folders.");
        } else {
            println!("Local changes made to files/folders:");
            changes.mutations.iter().for_each(|m| println!("{m}"));

            let num_of_changes = changes.mutations.len();
            println!("\nChanges found to local files/folders: {num_of_changes}");
        }
        Ok(())
    }

    /// Sync local changes made to files and folder with their version on the network,
    /// both pushing and pulling changes to/form the network.
    pub async fn sync(&mut self, upload_cfg: UploadCfg, make_data_public: bool) -> Result<()> {
        let ChangesToApply { folders, mutations } =
            self.scan_files_and_folders_for_changes(make_data_public)?;

        if mutations.is_empty() {
            println!("No local changes made to files/folders to be pushed to network.");
        } else {
            println!("Local changes made to files/folders to be synced with network:");
            mutations.iter().for_each(|m| println!("{m}"));
        }

        println!("Paying for folders hierarchy and uploading...");
        let synced_folders = self
            .pay_and_sync_folders(folders, upload_cfg, make_data_public)
            .await?;

        // mark root folder as created if it wasn't already
        if !self.root_folder_created {
            self.root_folder_created = true;
            store_root_folder_tracking_info(
                &self.meta_dir,
                self.root_folder_addr,
                self.root_folder_created,
            )?;
        }

        // update tracking information based on mutations detected locally
        for mutation in mutations {
            match mutation {
                Mutation::NewFile(tracking_info) | Mutation::NewFolder(tracking_info) => {
                    self.store_tracking_info(tracking_info)?;
                }
                Mutation::FileRemoved((_, meta_xorname))
                | Mutation::FolderRemoved((_, meta_xorname)) => {
                    self.remove_tracking_info(meta_xorname);
                }
                Mutation::FileContentChanged((meta_xorname, tracking_info)) => {
                    self.store_tracking_info(tracking_info)?;
                    self.remove_tracking_info(meta_xorname);
                }
            }
        }

        // download files/folders which are new in the synced folders
        let folders_to_download: Vec<_> = synced_folders
            .iter()
            .map(|(path, (folders_api, _))| {
                let folder_name: OsString = path.file_name().unwrap_or_default().into();
                (folder_name, folders_api.clone(), path.clone())
            })
            .collect();
        let mut updated_folders = self
            .download_folders_and_files(
                folders_to_download,
                upload_cfg.batch_size,
                upload_cfg.retry_strategy,
            )
            .await?;

        // Now let's check if any file/folder was removed remotely so we remove them locally from disk.
        // We do it in two phases, first we get rid of all dirs that were removed, then we go through
        // the files, this is to make sure we remove files which belong to nested folders being removed.
        let mut curr_tracking_info = read_tracking_info_from_disk(&self.meta_dir)?;
        curr_tracking_info.retain(|_, tracking_info| {
            if let FolderEntry::Folder(_) = tracking_info.metadata.content {
                !self.remove_tracking_if_not_found_in_folders(tracking_info, &mut updated_folders)
            } else {
                true
            }
        });
        curr_tracking_info.retain(|_, tracking_info| {
            if let FolderEntry::File(_) = tracking_info.metadata.content {
                !self.remove_tracking_if_not_found_in_folders(tracking_info, &mut updated_folders)
            } else {
                true
            }
        });

        self.curr_tracking_info = curr_tracking_info;

        Ok(())
    }

    // Private helpers

    // Generate the path relative to the user's root folder
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
            // we store the relative path so the root folder can be moved to
            // different locations/paths if desired by the user.
            file_path: self.get_relative_path(&file_path)?,
            meta_xorname,
            metadata,
            entry_hash,
        };

        meta_file.write_all(&rmp_serde::to_vec(&tracking_info)?)?;

        Ok(())
    }

    // Remove tracking information file for given xorname
    fn remove_tracking_info(&self, meta_xorname: XorName) {
        let metadata_file_path = self.meta_dir.join(hex::encode(meta_xorname));
        if let Err(err) = remove_file(&metadata_file_path) {
            println!("Failed to remove tracking info file {metadata_file_path:?}: {err}");
        }
    }

    // If the file/folder referenced by the tracking info provided is not part of the passed Folders
    // hierarchy, remove it from local disk along with its tracking information.
    // Returns whether the file/folder was removed.
    fn remove_tracking_if_not_found_in_folders(
        &self,
        tracking_info: &MetadataTrackingInfo,
        folders: &mut Folders,
    ) -> bool {
        let mut removed = false;
        let abs_path = self.files_dir.join(&tracking_info.file_path);
        match tracking_info.metadata.content {
            FolderEntry::Folder(_) => {
                match find_by_name_in_parent_folder(
                    &tracking_info.metadata.name,
                    &abs_path,
                    folders,
                ) {
                    Some(meta_xorname) => {
                        if meta_xorname != tracking_info.meta_xorname {
                            self.remove_tracking_info(tracking_info.meta_xorname);
                            removed = true;
                        }
                    }
                    None => {
                        if let Err(err) = remove_dir_all(&abs_path) {
                            trace!("Failed to remove directory {abs_path:?}: {err:?}");
                        }
                        self.remove_tracking_info(tracking_info.meta_xorname);
                        folders.remove(&abs_path);
                        removed = true;
                    }
                }
            }
            FolderEntry::File(_) => {
                match find_by_name_in_parent_folder(
                    &tracking_info.metadata.name,
                    &abs_path,
                    folders,
                ) {
                    Some(meta_xorname) => {
                        if meta_xorname != tracking_info.meta_xorname {
                            self.remove_tracking_info(tracking_info.meta_xorname);
                            removed = true;
                        }
                    }
                    None => {
                        if let Err(err) = remove_file(&abs_path) {
                            // this is expected if parent folder was just removed as part of this syncing flow.
                            trace!("Failed to remove file {abs_path:?}: {err:?}");
                        }
                        self.remove_tracking_info(tracking_info.meta_xorname);
                        removed = true;
                    }
                }
            }
        }

        removed
    }

    // Scan existing files and folders on disk, generating a report of all the detected
    // changes based on the tracking info kept locally.
    // If make_data_public is false the metadata chunks are encrypted.
    fn scan_files_and_folders_for_changes(&self, make_data_public: bool) -> Result<ChangesToApply> {
        // we don't use the local cache in order to realise of any changes made to files content.
        let mut chunk_manager = ChunkManager::new(&self.tracking_info_dir);
        chunk_manager.chunk_with_iter(self.iter_only_files(), false, false)?;

        let encryption_pk = if make_data_public {
            None
        } else {
            // we pass down the key to encrypt the metadata chunk of any new content detected.
            Some(self.client.signer_pk())
        };

        let mut changes = self.read_folders_hierarchy_from_disk(encryption_pk)?;

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
                                let (entry_hash, meta_xorname, metadata) = replace_item_in_folder(
                                    &mut parent_folder,
                                    tracking_info.entry_hash,
                                    chunked_file.file_name.clone(),
                                    chunked_file.data_map.clone(),
                                    encryption_pk,
                                )?;

                                changes.mutations.push(Mutation::FileContentChanged((
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
                            let (entry_hash, meta_xorname, metadata) = replace_item_in_folder(
                                &mut parent_folder,
                                tracking_info.entry_hash,
                                chunked_file.file_name.clone(),
                                chunked_file.data_map.clone(),
                                encryption_pk,
                            )?;
                            changes
                                .mutations
                                .push(Mutation::NewFile(MetadataTrackingInfo {
                                    file_path: file_path.to_path_buf(),
                                    meta_xorname,
                                    metadata,
                                    entry_hash,
                                }));
                        }
                    },
                    Ok(None) => {
                        let (entry_hash, meta_xorname, metadata) =
                            parent_folder.get_mut().0.add_file(
                                chunked_file.file_name.clone(),
                                chunked_file.data_map.clone(),
                                encryption_pk,
                            )?;
                        parent_folder.get_mut().1.has_new_entries();

                        changes
                            .mutations
                            .push(Mutation::NewFile(MetadataTrackingInfo {
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
        for (item_path, tracking_info) in self.curr_tracking_info.iter() {
            let abs_path = self.files_dir.join(item_path);
            match tracking_info.metadata.content {
                FolderEntry::Folder(_) => {
                    if !folders.contains_key(&abs_path) {
                        remove_from_parent(folders, &abs_path, tracking_info.entry_hash)?;
                        changes.mutations.push(Mutation::FolderRemoved((
                            abs_path,
                            tracking_info.meta_xorname,
                        )));
                    }
                }
                FolderEntry::File(_) => {
                    if chunk_manager
                        .iter_chunked_files()
                        .all(|chunked_file| chunked_file.file_path != abs_path)
                    {
                        remove_from_parent(folders, &abs_path, tracking_info.entry_hash)?;
                        changes.mutations.push(Mutation::FileRemoved((
                            abs_path,
                            tracking_info.meta_xorname,
                        )));
                    }
                }
            }
        }

        Ok(changes)
    }

    // Build Folders hierarchy from the set files dir. The metadata chunk of every new folder
    // will be encrpyted if an encrpytion key has been provided.
    fn read_folders_hierarchy_from_disk(
        &self,
        encryption_pk: Option<PublicKey>,
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
            let (folder, folder_change) = changes
                .folders
                .entry(dir_path.clone())
                .or_insert(self.find_folder_in_tracking_info(&dir_path)?)
                .clone();
            let curr_folder_addr = *folder.address();

            if depth > 0 {
                let (parent_folder, parent_folder_change) = changes
                    .folders
                    .entry(parent.clone())
                    .or_insert(self.find_folder_in_tracking_info(&parent)?);

                if folder_change.is_new_folder() {
                    let (entry_hash, meta_xorname, metadata) =
                        parent_folder.add_folder(dir_name, curr_folder_addr, encryption_pk)?;
                    parent_folder_change.has_new_entries();

                    changes
                        .mutations
                        .push(Mutation::NewFolder(MetadataTrackingInfo {
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
        Ok(self.curr_tracking_info.get(&path))
    }

    // Instantiate a FolderApi based on local tracking info for given folder item
    fn find_folder_in_tracking_info(&self, path: &Path) -> Result<(FoldersApi, FolderChange)> {
        let mut folder_change = FolderChange::NewFolder;
        let address = if path == self.files_dir {
            if self.root_folder_created {
                folder_change = FolderChange::NoChange;
            }
            Some(self.root_folder_addr)
        } else {
            self.get_tracking_info(path)?.and_then(|tracking_info| {
                match tracking_info.metadata.content {
                    FolderEntry::Folder(addr) => {
                        folder_change = FolderChange::NoChange;
                        Some(addr)
                    }
                    FolderEntry::File(_) => None,
                }
            })
        };

        let folders_api = FoldersApi::new(self.client.clone(), &self.wallet_dir, address)?;
        Ok((folders_api, folder_change))
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

    // Pay and upload all the files and folder.
    async fn pay_and_sync_folders(
        &self,
        folders: Folders,
        upload_cfg: UploadCfg,
        make_data_public: bool,
    ) -> Result<Folders> {
        let files_uploader = FilesUploader::new(self.client.clone(), self.wallet_dir.clone())
            .set_upload_cfg(upload_cfg)
            .set_make_data_public(make_data_public)
            .insert_entries(self.iter_only_files());
        let _summary = files_uploader.start_upload().await?;

        // Let's make the storage payment for Folders
        let new_wallet = MainSecretKey::random();
        let wallet = HotWallet::create_from_key(&self.wallet_dir, new_wallet)?;

        let mut wallet_client = WalletClient::new(self.client.clone(), wallet);
        let mut net_addresses = vec![];
        let mut new_folders = 0;
        // let's collect list of addresses we need to pay for
        folders.iter().for_each(|(_, (folder, folder_change))| {
            if folder_change.is_new_folder() {
                net_addresses.push(folder.as_net_addr());
                new_folders += 1;
            }
            net_addresses.extend(folder.meta_addrs_to_pay());
        });

        let payment_result = wallet_client
            .pay_for_storage(net_addresses.into_iter())
            .await?;
        match payment_result
            .storage_cost
            .checked_add(payment_result.royalty_fees)
        {
            Some(cost) => {
                let balance = wallet_client.balance();
                println!("Made payment of {cost} for {new_folders} Folders. New balance: {balance}",)
            }
            None => bail!("Failed to calculate total payment cost"),
        }

        // Sync Folders concurrently now that payments have been made.
        let mut tasks = JoinSet::new();
        for (path, (mut folder, folder_change)) in folders {
            let op = if folder_change.is_new_folder() {
                "Creation"
            } else {
                "Syncing"
            };

            tasks.spawn(async move {
                match folder.sync(upload_cfg).await {
                    Ok(()) => {
                        println!(
                            "{op} of Folder (for {path:?}) succeeded. Address: {}",
                            folder.address().to_hex()
                        );
                    }
                    Err(err) => {
                        println!("{op} of Folder (for {path:?}) failed: {err}")
                    }
                }
                (path, folder, folder_change)
            });
        }

        let mut synced_folders = Folders::new();
        while let Some(res) = tasks.join_next().await {
            match res {
                Ok((path, folder, c)) => {
                    synced_folders.insert(path, (folder, c));
                }
                Err(err) => {
                    println!("Failed to sync/create a Folder with/on the network: {err:?}");
                }
            }
        }

        Ok(synced_folders)
    }

    // Download a Folders and their files from the network and generate tracking info
    async fn download_folders_and_files(
        &self,
        mut folders_to_download: Vec<(OsString, FoldersApi, PathBuf)>,
        batch_size: usize,
        retry_strategy: RetryStrategy,
    ) -> Result<Folders> {
        let mut files_to_download = vec![];
        let mut updated_folders = Folders::new();
        while let Some((name, mut folders_api, target_path)) = folders_to_download.pop() {
            if updated_folders.contains_key(&target_path) {
                // we've already downloaded this Folder
                continue;
            }

            println!(
                "Downloading Folder {name:?} from {}",
                folders_api.address().to_hex()
            );
            self.download_folder_from_network(
                &target_path,
                &mut folders_api,
                &mut files_to_download,
                &mut folders_to_download,
            )
            .await?;
            updated_folders.insert(target_path, (folders_api, FolderChange::NoChange));
        }

        let files_api: FilesApi = FilesApi::new(self.client.clone(), self.files_dir.clone());
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

        Ok(updated_folders)
    }

    // Download a Folder from the network and generate tracking info
    async fn download_folder_from_network(
        &self,
        target_path: &Path,
        folders_api: &mut FoldersApi,
        files_to_download: &mut Vec<(OsString, Chunk, PathBuf)>,
        folders_to_download: &mut Vec<(OsString, FoldersApi, PathBuf)>,
    ) -> Result<()> {
        for (entry_hash, (meta_xorname, metadata)) in folders_api.entries().await?.into_iter() {
            let name = metadata.name.clone();
            let item_path = target_path.join(name.clone());
            if let Ok(Some(tracking_info)) = self.get_tracking_info(&item_path) {
                if tracking_info.meta_xorname == meta_xorname {
                    // thus we already have this same file/folder locally
                    continue;
                }
            }

            match &metadata.content {
                FolderEntry::File(data_map_chunk) => {
                    files_to_download.push((
                        name.clone().into(),
                        data_map_chunk.clone(),
                        target_path.to_path_buf(),
                    ));
                    let _ = File::create(&item_path)?;
                }
                FolderEntry::Folder(subfolder_addr) => {
                    let folders_api = FoldersApi::retrieve(
                        self.client.clone(),
                        &self.wallet_dir,
                        *subfolder_addr,
                    )
                    .await?;

                    folders_to_download.push((name.clone().into(), folders_api, item_path.clone()));
                    create_dir_all(&item_path)?;
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

// Given an absolute path, find the Folder containing such item, and remove it from its entries.
fn remove_from_parent(folders: &mut Folders, path: &Path, entry_hash: EntryHash) -> Result<()> {
    if let Some((parent_folder, folder_change)) = path.parent().and_then(|p| folders.get_mut(p)) {
        folder_change.has_new_entries();
        parent_folder.remove_item(entry_hash)?;
    }
    Ok(())
}

// Replace a file/folder item from a given Folder (passed in as a container's OccupiedEntry').
// The metadata chunk of the new item (folder/file) will be encrpyted if a key has been provided.
fn replace_item_in_folder(
    folder: &mut OccupiedEntry<'_, PathBuf, (FoldersApi, FolderChange)>,
    entry_hash: EntryHash,
    file_name: OsString,
    data_map: Chunk,
    encryption_pk: Option<PublicKey>,
) -> Result<(EntryHash, XorName, Metadata)> {
    let (ref mut folders_api, ref mut folder_change) = folder.get_mut();
    folder_change.has_new_entries();
    let res = folders_api.replace_file(
        entry_hash,
        file_name.clone(),
        data_map.clone(),
        encryption_pk,
    )?;
    Ok(res)
}

// Search for a file/folder item in its parent Folder by its name, returning its metadata chunk xorname.
fn find_by_name_in_parent_folder(name: &str, path: &Path, folders: &Folders) -> Option<XorName> {
    path.parent()
        .and_then(|parent| folders.get(parent))
        .and_then(|(folder, _)| folder.find_by_name(name))
        .map(|(meta_xorname, _)| *meta_xorname)
}

// Using the provided root SK, derive client signer SK and the root Folder address from it.
// It returns the Client updated with the derived signing key set, along with the derived Register address.
// TODO: use eip2333 path for deriving keys and address.
fn derive_keys_and_address(
    mut client: Client,
    root_sk: &MainSecretKey,
) -> (Client, RegisterAddress) {
    // Set the client signer SK as a derived key from the root key. This will
    // be used for signing operations and also for encrypting metadata chunks.
    let signer_sk = root_sk
        .derive_key(&ACC_PACKET_OWNER_DERIVATION_INDEX)
        .secret_key();
    client.set_signer_key(signer_sk);

    // Derive a key from the root key to generate the root Folder xorname, and use
    // the client signer's corresponding PK as the owner of it.
    let derived_pk = root_sk
        .derive_key(&ACC_PACKET_ADDR_DERIVATION_INDEX)
        .secret_key()
        .public_key();
    let root_folder_addr = RegisterAddress::new(
        XorName::from_content(&derived_pk.to_bytes()),
        client.signer_pk(),
    );

    (client, root_folder_addr)
}

#[cfg(test)]
mod tests {
    // All tests require a network running so Clients can be instantiated.

    use crate::acc_packet::{
        derive_keys_and_address, RECOVERY_SEED_FILENAME, SAFE_TRACKING_CHANGES_DIR,
    };

    use super::{
        read_root_folder_addr, read_tracking_info_from_disk, AccountPacket, Metadata,
        MetadataTrackingInfo, Mutation, ACC_PACKET_ADDR_DERIVATION_INDEX,
        ACC_PACKET_OWNER_DERIVATION_INDEX,
    };
    use rand::{thread_rng, Rng};
    use sn_client::{
        protocol::storage::{Chunk, RetryStrategy},
        registers::{EntryHash, RegisterAddress},
        test_utils::{get_funded_wallet, get_new_client, random_file_chunk},
        transfers::MainSecretKey,
        FolderEntry, UploadCfg, BATCH_SIZE,
    };

    use bls::SecretKey;
    use bytes::Bytes;
    use eyre::{bail, eyre, Result};
    use std::{
        collections::{BTreeMap, BTreeSet},
        fs::{create_dir_all, remove_dir_all, remove_file, File, OpenOptions},
        io::{Read, Write},
        path::{Path, PathBuf},
    };
    use xor_name::XorName;

    const SYNC_OPTS: (UploadCfg, bool) = {
        let cfg = UploadCfg {
            verify_store: true,
            batch_size: BATCH_SIZE,
            retry_strategy: RetryStrategy::Quick,
            show_holders: false,
            max_repayments_for_failed_data: 1,
            collect_registers: false,
        };
        let make_data_public = false;
        (cfg, make_data_public)
    };

    #[tokio::test]
    async fn test_acc_packet_private_helpers() -> Result<()> {
        let client = get_new_client(SecretKey::random()).await?;
        let root_sk = MainSecretKey::random();

        let tmp_dir = tempfile::tempdir()?;
        let wallet_dir = tmp_dir.path();
        let files_path = tmp_dir.path().join("myfiles");
        create_dir_all(&files_path)?;

        let owner_pk = root_sk
            .derive_key(&ACC_PACKET_OWNER_DERIVATION_INDEX)
            .secret_key()
            .public_key();
        let xorname = XorName::from_content(
            &root_sk
                .derive_key(&ACC_PACKET_ADDR_DERIVATION_INDEX)
                .secret_key()
                .public_key()
                .to_bytes(),
        );
        let expected_folder_addr = RegisterAddress::new(xorname, owner_pk);

        let acc_packet =
            AccountPacket::init(client.clone(), wallet_dir, &files_path, &root_sk, None)?;
        assert_eq!(
            derive_keys_and_address(client, &root_sk).1,
            expected_folder_addr
        );
        assert_eq!(acc_packet.root_folder_addr(), expected_folder_addr);

        let mut test_files = create_test_files_on_disk(&files_path)?;
        let mut rng = rand::thread_rng();
        let dummy_metadata = Metadata {
            name: "dummy".to_string(),
            content: FolderEntry::File(Chunk::new(Bytes::new())),
        };
        for (relative_path, _) in test_files.iter() {
            let abs_path = files_path.join(relative_path);

            // test helper which calculates relative paths based on root files dir of acc packet
            assert!(
                matches!(acc_packet.get_relative_path(&abs_path), Ok(p) if &p == relative_path),
                "AccountPacket::get_relative_path helper returned invalid path"
            );

            // let's test helper to store tracking info
            // use just dummy/invalid metadata and meta-xorname since we won't verify it
            let meta_xorname = XorName::random(&mut rng);
            acc_packet.store_tracking_info(MetadataTrackingInfo {
                file_path: abs_path,
                meta_xorname,
                metadata: dummy_metadata.clone(),
                entry_hash: EntryHash::default(),
            })?;
            assert!(acc_packet.meta_dir.join(hex::encode(meta_xorname)).exists());
        }

        // let's test helpers to read and remove tracking info
        let tracking_info = read_tracking_info_from_disk(&acc_packet.meta_dir)?;
        assert_eq!(tracking_info.len(), test_files.len());
        for (abs_path, info) in tracking_info.iter() {
            assert!(test_files.remove(abs_path).is_some());
            acc_packet.remove_tracking_info(info.meta_xorname);
            assert!(!acc_packet
                .meta_dir
                .join(hex::encode(info.meta_xorname))
                .exists());
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_acc_packet_from_empty_dir() -> Result<()> {
        let client = get_new_client(SecretKey::random()).await?;
        let root_sk = MainSecretKey::random();

        let tmp_dir = tempfile::tempdir()?;
        let wallet_dir = tmp_dir.path();
        let _ = get_funded_wallet(&client, wallet_dir).await?;

        let src_files_path = tmp_dir.path().join("myaccpacketempty");
        create_dir_all(&src_files_path)?;

        let mut acc_packet =
            AccountPacket::init(client.clone(), wallet_dir, &src_files_path, &root_sk, None)?;

        // let's sync up with the network from the original empty account packet
        acc_packet.sync(SYNC_OPTS.0, SYNC_OPTS.1).await?;

        let clone_files_path = tmp_dir.path().join("myaccpacketempty-clone");
        let cloned_acc_packet = AccountPacket::retrieve_folders(
            &client,
            wallet_dir,
            &root_sk,
            None,
            &clone_files_path,
            BATCH_SIZE,
            RetryStrategy::Quick,
        )
        .await?;

        // let's verify both the original and cloned packets are empty
        check_files_and_dirs_match(&acc_packet, &cloned_acc_packet, BTreeMap::new())?;
        check_tracking_info_match(&acc_packet, &cloned_acc_packet, BTreeMap::new())?;

        Ok(())
    }

    #[tokio::test]
    async fn test_acc_packet_upload_download() -> Result<()> {
        let client = get_new_client(SecretKey::random()).await?;
        let root_sk = MainSecretKey::random();

        let tmp_dir = tempfile::tempdir()?;
        let wallet_dir = tmp_dir.path();
        let _ = get_funded_wallet(&client, wallet_dir).await?;

        let src_files_path = tmp_dir.path().join("myaccpacket");
        let expected_files = create_test_files_on_disk(&src_files_path)?;

        let mut acc_packet =
            AccountPacket::init(client.clone(), wallet_dir, &src_files_path, &root_sk, None)?;

        acc_packet.sync(SYNC_OPTS.0, SYNC_OPTS.1).await?;

        let download_files_path = tmp_dir.path().join("myaccpacket-downloaded");

        let downloaded_acc_packet = AccountPacket::retrieve_folders(
            &client,
            wallet_dir,
            &root_sk,
            None,
            &download_files_path,
            BATCH_SIZE,
            RetryStrategy::Quick,
        )
        .await?;

        check_files_and_dirs_match(&acc_packet, &downloaded_acc_packet, expected_files.clone())?;
        check_tracking_info_match(&acc_packet, &downloaded_acc_packet, expected_files)?;

        Ok(())
    }

    #[tokio::test]
    async fn test_acc_packet_scan_files_and_folders_changes() -> Result<()> {
        let client = get_new_client(SecretKey::random()).await?;
        let root_sk = MainSecretKey::random();

        let tmp_dir = tempfile::tempdir()?;
        let wallet_dir = tmp_dir.path();
        let _ = get_funded_wallet(&client, wallet_dir).await?;

        let files_path = tmp_dir.path().join("myaccpacket-to-scan");
        let mut test_files = create_test_files_on_disk(&files_path)?;
        let files_path = files_path.canonicalize()?;

        let mut acc_packet =
            AccountPacket::init(client.clone(), wallet_dir, &files_path, &root_sk, None)?;

        let changes = acc_packet.scan_files_and_folders_for_changes(false)?;
        // verify changes detected
        assert_eq!(changes.mutations.len(), 4);
        assert!(changes.mutations.iter().all(|mutation| {
            matches!(mutation, Mutation::NewFile(i) if i.file_path == files_path.join("file0.txt"))
            || matches!(mutation, Mutation::NewFile(i) if i.file_path == files_path.join("dir1").join("file1.txt"))
            || matches!(mutation, Mutation::NewFolder(i) if i.file_path == files_path.join("dir1"))
            || matches!(mutation, Mutation::NewFolder(i) if i.file_path == files_path.join("dir2"))
        }), "at least one of the mutations detected was unexpected/incorrect");

        acc_packet.sync(SYNC_OPTS.0, SYNC_OPTS.1).await?;

        // let's make some mutations/changes
        mutate_test_files_on_disk(&files_path, &mut test_files)?;

        let changes = acc_packet.scan_files_and_folders_for_changes(false)?;
        // verify new changes detected
        assert_eq!(changes.mutations.len(), 8);
        assert!(changes.mutations.iter().all(|mutation| {
            matches!(mutation, Mutation::FileContentChanged((_,i)) if i.file_path == files_path.join("file0.txt"))
            || matches!(mutation, Mutation::FileRemoved((p, _)) if p == &files_path.join("dir1").join("file1.txt"))
            || matches!(mutation, Mutation::FolderRemoved((p,_)) if p == &files_path.join("dir2"))
            || matches!(mutation, Mutation::NewFolder(i) if i.file_path == files_path.join("dir3"))
            || matches!(mutation, Mutation::NewFolder(i) if i.file_path == files_path.join("dir3").join("dir3_1"))
            || matches!(mutation, Mutation::NewFile(i) if i.file_path == files_path.join("dir3").join("dir3_1").join("file3.txt"))
            || matches!(mutation, Mutation::NewFolder(i) if i.file_path == files_path.join("dir4"))
            || matches!(mutation, Mutation::NewFolder(i) if i.file_path == files_path.join("dir4").join("dir4_1"))
        }), "at least one of the mutations detected was unexpected/incorrect");

        Ok(())
    }

    #[tokio::test]
    async fn test_acc_packet_sync_mutations() -> Result<()> {
        let client = get_new_client(SecretKey::random()).await?;
        let root_sk = MainSecretKey::random();

        let tmp_dir = tempfile::tempdir()?;
        let wallet_dir = tmp_dir.path();
        let _ = get_funded_wallet(&client, wallet_dir).await?;

        let src_files_path = tmp_dir.path().join("myaccpackettosync");
        let mut expected_files = create_test_files_on_disk(&src_files_path)?;

        let mut acc_packet =
            AccountPacket::init(client.clone(), wallet_dir, &src_files_path, &root_sk, None)?;

        acc_packet.sync(SYNC_OPTS.0, SYNC_OPTS.1).await?;

        let clone_files_path = tmp_dir.path().join("myaccpackettosync-clone");
        let mut cloned_acc_packet = AccountPacket::retrieve_folders(
            &client,
            wallet_dir,
            &root_sk,
            None,
            &clone_files_path,
            BATCH_SIZE,
            RetryStrategy::Quick,
        )
        .await?;

        // let's make mutations to the clone:
        mutate_test_files_on_disk(&clone_files_path, &mut expected_files)?;

        // and finally, sync the clone up with the network
        cloned_acc_packet.sync(SYNC_OPTS.0, SYNC_OPTS.1).await?;

        // let's sync up with the network from the original account packet to merge
        // changes made earlier from the cloned version
        acc_packet.sync(SYNC_OPTS.0, SYNC_OPTS.1).await?;

        // let's verify both the original and cloned packets contain the same content
        check_files_and_dirs_match(&acc_packet, &cloned_acc_packet, expected_files.clone())?;
        check_tracking_info_match(&acc_packet, &cloned_acc_packet, expected_files)?;

        Ok(())
    }

    // Acc-packets can be moved to different locations on local disk without affecting their tracking info.
    // We disable this test for Windows since in CI the use of std::fs::rename gives a permissions issue.
    #[cfg(any(target_os = "linux", target_os = "linux"))]
    #[tokio::test]
    async fn test_acc_packet_moved_folder() -> Result<()> {
        let client = get_new_client(SecretKey::random()).await?;
        let root_sk = MainSecretKey::random();

        let tmp_dir = tempfile::tempdir()?;
        let wallet_dir = tmp_dir.path();
        let _ = get_funded_wallet(&client, wallet_dir).await?;

        let src_files_path = tmp_dir.path().join("myaccpacket-to-move");
        let mut test_files = create_test_files_on_disk(&src_files_path)?;

        let mut acc_packet =
            AccountPacket::init(client.clone(), wallet_dir, &src_files_path, &root_sk, None)?;

        acc_packet.sync(SYNC_OPTS.0, SYNC_OPTS.1).await?;

        // let's make just one mutation before moving the dir to another disk location
        let new_chunk = random_file_chunk();
        let file2modify = Path::new("dir1").join("file1.txt");
        OpenOptions::new()
            .write(true)
            .open(src_files_path.join(&file2modify))?
            .write_all(new_chunk.value())?;
        test_files.insert(file2modify, Some(new_chunk));

        // let's now move it to another disk location
        let moved_files_path = tmp_dir.path().join("myaccpacket-moved");
        create_dir_all(&moved_files_path)?;
        std::fs::rename(src_files_path, &moved_files_path)?;
        let moved_files_path = moved_files_path.canonicalize()?;

        let moved_acc_packet =
            AccountPacket::from_path(client.clone(), wallet_dir, &moved_files_path, None)?;

        // verify only one change is detected still after moved to another location on disk
        let changes = moved_acc_packet.scan_files_and_folders_for_changes(false)?;
        assert_eq!(changes.mutations.len(), 1);
        assert_eq!(changes.mutations.first().map(|mutation| {
            matches!(mutation, Mutation::FileContentChanged((_,i)) if i.file_path == moved_files_path.join("dir1").join("file1.txt"))
        }), Some(true));

        check_tracking_info_match(&moved_acc_packet, &moved_acc_packet, test_files)?;

        Ok(())
    }

    #[tokio::test]
    async fn test_acc_packet_derived_address() -> Result<()> {
        let client = get_new_client(SecretKey::random()).await?;
        let root_sk = MainSecretKey::random();

        let tmp_dir = tempfile::tempdir()?;
        let wallet_dir = tmp_dir.path();
        let _ = get_funded_wallet(&client, wallet_dir).await?;

        let files_path = tmp_dir.path().join("myaccpacket-unencrypted-metadata");
        let _ = create_test_files_on_disk(&files_path)?;

        let mut acc_packet =
            AccountPacket::init(client.clone(), wallet_dir, &files_path, &root_sk, None)?;
        acc_packet.sync(SYNC_OPTS.0, SYNC_OPTS.1).await?;

        // try to download Folder with a different root SK should fail since it
        // will derive a different addresse than the one used for creating it
        let download_files_path = tmp_dir.path().join("myaccpacket-downloaded");
        let other_root_sk = MainSecretKey::random();

        if AccountPacket::retrieve_folders(
            &client,
            wallet_dir,
            &other_root_sk,
            None,
            &download_files_path,
            BATCH_SIZE,
            RetryStrategy::Quick,
        )
        .await
        .is_ok()
        {
            bail!("acc-packet retrieval succeeded unexpectedly");
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_acc_packet_recovery_seed_encryption() -> Result<()> {
        let client = get_new_client(SecretKey::random()).await?;
        let root_sk = MainSecretKey::random();
        let tmp_dir = tempfile::tempdir()?;
        let wallet_dir = tmp_dir.path();

        // let's first test with unencrypted recovery seed
        let src_files_path = tmp_dir.path().join("myaccpacket_unencrypted_seed");
        create_dir_all(&src_files_path)?;
        let _ = AccountPacket::init(client.clone(), wallet_dir, &src_files_path, &root_sk, None)?;
        let _ = AccountPacket::from_path(client.clone(), wallet_dir, &src_files_path, None)?;

        let bytes = std::fs::read(
            src_files_path
                .join(SAFE_TRACKING_CHANGES_DIR)
                .join(RECOVERY_SEED_FILENAME),
        )?;
        assert_eq!(bytes, root_sk.to_bytes());

        if AccountPacket::from_path(
            client.clone(),
            wallet_dir,
            &src_files_path,
            Some(b"123456789"),
        )
        .is_ok()
        {
            bail!("acc-packet loading with a password succeeded unexpectedly");
        }

        // let's now test with encrypted recovery seed
        let src_files_path = tmp_dir.path().join("myaccpacket_encrypted_seed");
        create_dir_all(&src_files_path)?;
        let mut rng = thread_rng();
        let password: [u8; 32] = rng.gen();
        let incorrect_password: [u8; 32] = rng.gen();

        let _ = AccountPacket::init(
            client.clone(),
            wallet_dir,
            &src_files_path,
            &root_sk,
            Some(&password),
        )?;

        if AccountPacket::from_path(client.clone(), wallet_dir, &src_files_path, None).is_ok() {
            bail!("acc-packet loading without a password succeeded unexpectedly");
        }

        if AccountPacket::from_path(
            client.clone(),
            wallet_dir,
            &src_files_path,
            Some(&incorrect_password),
        )
        .is_ok()
        {
            bail!("acc-packet loading with incorrect password succeeded unexpectedly");
        }

        let _ =
            AccountPacket::from_path(client.clone(), wallet_dir, &src_files_path, Some(&password))?;

        let bytes = std::fs::read(
            src_files_path
                .join(SAFE_TRACKING_CHANGES_DIR)
                .join(RECOVERY_SEED_FILENAME),
        )?;
        assert!(!bytes.is_empty());
        assert_ne!(bytes, root_sk.to_bytes());

        Ok(())
    }

    // Helpers functions to generate and verify test data

    // Create a hard-coded set of test files and dirs on disk
    fn create_test_files_on_disk(base_path: &Path) -> Result<BTreeMap<PathBuf, Option<Chunk>>> {
        // let's create a hierarchy with dirs and files with random content
        let mut files = BTreeMap::new();
        files.insert(
            Path::new("file0.txt").to_path_buf(),
            Some(random_file_chunk()),
        );
        files.insert(
            Path::new("dir1").join("file1.txt"),
            Some(random_file_chunk()),
        );
        files.insert(Path::new("dir2").to_path_buf(), None);

        for (path, chunk) in files.iter() {
            let full_path = base_path.join(path);
            if let Some(chunk) = chunk {
                // it's a file, thus we create it and store its chunk bytes
                create_dir_all(full_path.parent().expect("invalid path for test file"))?;
                let mut file = File::create(full_path)?;
                file.write_all(chunk.value())?;
            } else {
                // it's a dir, and it shall be empty
                create_dir_all(full_path)?;
            }
        }
        Ok(files)
    }

    // Apply a hard-coded set of mutations to test files and dirs on disk
    fn mutate_test_files_on_disk(
        path: &Path,
        test_files: &mut BTreeMap<PathBuf, Option<Chunk>>,
    ) -> Result<()> {
        // - modify the content of a file
        let new_chunk = random_file_chunk();
        let file2modify = Path::new("file0.txt");
        OpenOptions::new()
            .write(true)
            .open(path.join(file2modify))?
            .write_all(new_chunk.value())?;
        test_files.insert(file2modify.to_path_buf(), Some(new_chunk));
        // - remove one of the files
        let file2remove = Path::new("dir1").join("file1.txt");
        remove_file(path.join(&file2remove))?;
        test_files.remove(&file2remove);
        // we need to keep the empty dir within the list of expected files though
        test_files.insert(Path::new("dir1").to_path_buf(), None);
        // - remove one of the dirs
        let dir2remove = Path::new("dir2");
        remove_dir_all(path.join(dir2remove))?;
        test_files.remove(dir2remove);
        // - create new file within subdirs
        create_dir_all(path.join("dir3").join("dir3_1"))?;
        let file2create = Path::new("dir3").join("dir3_1").join("file3.txt");
        let mut file = File::create(path.join(&file2create))?;
        let new_chunk = random_file_chunk();
        file.write_all(new_chunk.value())?;
        test_files.insert(file2create, Some(new_chunk));
        // - create new subdirs
        let dir2create = Path::new("dir4").join("dir4_1");
        create_dir_all(path.join(&dir2create))?;
        test_files.insert(dir2create.to_path_buf(), None);

        Ok(())
    }

    // Helper to check if a dir is empty
    fn is_empty_dir(path: &Path) -> bool {
        path.read_dir()
            .map(|mut i| i.next().is_none())
            .unwrap_or(false)
    }

    // Collect list of files and empty dirs, to be used for comparing in tests
    fn list_of_files_and_empty_dirs(acc_packet: &AccountPacket) -> BTreeSet<PathBuf> {
        acc_packet
            .iter_only_files()
            .chain(acc_packet.iter_only_dirs())
            .flat_map(|file_entry| {
                let path = file_entry.path();
                if path.is_dir() && !is_empty_dir(path) {
                    bail!("we skip non empty dirs");
                }

                acc_packet.get_relative_path(path)
            })
            .collect()
    }

    // Check both acc packets kept the same set of tracking information locally
    fn check_tracking_info_match(
        src_packet: &AccountPacket,
        target_packet: &AccountPacket,
        mut expected_files: BTreeMap<PathBuf, Option<Chunk>>,
    ) -> Result<()> {
        let root_addr = src_packet.root_folder_addr();
        assert_eq!(
            read_root_folder_addr(&src_packet.meta_dir)?,
            (root_addr, true),
            "Root folder address doesn't match in source directory tracking info."
        );
        assert_eq!(
            read_root_folder_addr(&target_packet.meta_dir)?,
            (root_addr, true),
            "Root folder address doesn't match in target directory tracking info."
        );

        let src_tracking_info = read_tracking_info_from_disk(&src_packet.meta_dir)?;
        let mut target_tracking_info = read_tracking_info_from_disk(&target_packet.meta_dir)?;

        for (path, src_tracking_info) in src_tracking_info {
            match target_tracking_info.remove(&path) {
                None => {
                    bail!("Tracking info found in source is missing in target directory for file/dir: {path:?}")
                }
                Some(info) => {
                    if info != src_tracking_info {
                        bail!("Different tracking info kept in source and target for file/dir: {path:?}");
                    }
                }
            }

            let abs_path = src_packet.files_dir.join(&path);
            if abs_path.is_dir() {
                assert_eq!(src_tracking_info.file_path, path,
                    "Incorrect path in tracking info found in source and target directories for dir: {path:?}");
                assert!(matches!(src_tracking_info.metadata.content, FolderEntry::Folder(_)),
                    "Incorrect tracking info found in source and target directories for dir: {path:?}");
                // if it's an empty dir we shall find it in the list of expected files
                if is_empty_dir(&abs_path) {
                    let _ = expected_files.remove(&path).ok_or_else(|| {
                        eyre!(
                            "Unexpected tracking info found on source and target directories for dir: {path:?}"
                        )
                    })?;
                }
            } else {
                let chunk = expected_files.remove(&path).ok_or_else(|| {
                    eyre!(
                        "Unexpected tracking info found on source and target directories for file: {path:?}"
                    )
                })?;

                if chunk.is_some() {
                    assert!(matches!(src_tracking_info.metadata.content, FolderEntry::File(_)),
                        "Tracking info found in source and target directories don't match the file: {path:?}");
                } else {
                    assert!(matches!(src_tracking_info.metadata.content, FolderEntry::Folder(_)),
                        "Tracking info found in source and target directories don't match the dir: {path:?}");
                }
            }
        }

        if !target_tracking_info.is_empty() {
            bail!("Tracking info found in target directory but missing in source directory: {target_tracking_info:?}");
        }
        if !expected_files.is_empty() {
            bail!("Some expected file/dir/s are lacking their tracking info in source or target directories: {expected_files:?}");
        }

        Ok(())
    }

    // Check both dirs have the same set of files and folders and no more
    fn check_files_and_dirs_match(
        src_packet: &AccountPacket,
        target_packet: &AccountPacket,
        mut expected_files: BTreeMap<PathBuf, Option<Chunk>>,
    ) -> Result<()> {
        // let's collect all paths in target acc packet, i.e. files and empty dirs paths
        let mut target_packet_files: BTreeSet<PathBuf> =
            list_of_files_and_empty_dirs(target_packet);

        // let's now compare those paths in target acc packet with those in source acc packet
        for relative_path in list_of_files_and_empty_dirs(src_packet) {
            if !target_packet_files.remove(&relative_path) {
                bail!("File/dir found in source is missing in target directory: {relative_path:?}");
            }

            let src_path = src_packet.files_dir.join(&relative_path);
            let target_path = target_packet.files_dir.join(&relative_path);

            let chunk = expected_files.remove(&relative_path).ok_or_else(|| {
                eyre!("Unexpected file/dir found on source and target directories: {src_path:?}")
            })?;

            if let Some(chunk) = chunk {
                // it's a file, let's compare their content
                let mut src_file = File::open(&src_path)
                    .map_err(|err| eyre!("couldn't open source file {src_path:?}: {err:?}"))?;
                let mut target_file = File::open(&target_path)
                    .map_err(|err| eyre!("couldn't open target file {target_path:?}: {err:?}"))?;

                let mut src_content = Vec::new();
                src_file
                    .read_to_end(&mut src_content)
                    .expect("couldn't read source file");
                let mut target_content = Vec::new();
                target_file
                    .read_to_end(&mut target_content)
                    .expect("couldn't read target file");

                assert_eq!(
                    src_content,
                    chunk.value().slice(..),
                    "source file content doesn't match with expected"
                );
                assert_eq!(
                    target_content,
                    chunk.value().slice(..),
                    "target file content doesn't match with expected"
                );
            } else {
                // it's a dir, let's check they exist as dirs
                assert!(src_path.is_dir(), "source path is not a dir {src_path:?}");
                assert!(
                    target_path.is_dir(),
                    "target path is not a dir {target_path:?}"
                );
            }
        }

        if !target_packet_files.is_empty() {
            bail!("File/dir/s found in target directory but missing in source directory: {target_packet_files:?}");
        }
        if !expected_files.is_empty() {
            bail!("Some expected file/dir/s were not found in source or target directories: {expected_files:?}");
        }

        Ok(())
    }
}
