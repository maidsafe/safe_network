// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use bls::{SecretKey, SK_SIZE};
use sn_client::{
    protocol::storage::RegisterAddress, registers::EntryHash, transfers::MainSecretKey, FoldersApi,
    Metadata,
};

use color_eyre::{eyre::eyre, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fmt,
    fs::{create_dir_all, File},
    io::Write,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;
use xor_name::XorName;

// Name of hidden folder where tracking information and metadata is locally stored.
pub(super) const SAFE_TRACKING_CHANGES_DIR: &str = ".safe";

// Subfolder where files metadata will be cached
pub(super) const METADATA_CACHE_DIR: &str = "metadata";

// Name of the file where metadata about root folder is locally cached.
pub(super) const ROOT_FOLDER_METADATA_FILENAME: &str = "root_folder.addr";

// Name of the file where the recovery secret/seed is locally cached.
pub(crate) const RECOVERY_SEED_FILENAME: &str = "recovery_seed";

// Container to keep track in memory what changes are detected in local Folders hierarchy and files.
pub(super) type Folders = BTreeMap<PathBuf, (FoldersApi, FolderChange)>;

// Type of local changes detected to a Folder
#[derive(Clone, Debug, PartialEq)]
pub(super) enum FolderChange {
    NoChange,
    NewFolder,
    NewEntries,
}

impl FolderChange {
    /// Returns true if it's currently set to NewFolder.
    pub fn is_new_folder(&self) -> bool {
        self == &Self::NewFolder
    }

    /// If it's currently set to NoChange then switch it to NewEntries.
    /// Otherwise we don't need to change it as the entire Folder will need to be uploaded.
    pub fn has_new_entries(&mut self) {
        if self == &Self::NoChange {
            *self = Self::NewEntries;
        }
    }
}

// Changes detected locally which eventually can be applied and upload to network.
#[derive(Default)]
pub(super) struct ChangesToApply {
    pub folders: Folders,
    pub mutations: Vec<Mutation>,
}

// Type of mutation detected locally.
#[derive(Debug)]
pub(super) enum Mutation {
    NewFile(MetadataTrackingInfo),
    FileRemoved((PathBuf, XorName)),
    FileContentChanged((XorName, MetadataTrackingInfo)),
    NewFolder(MetadataTrackingInfo),
    FolderRemoved((PathBuf, XorName)),
}

impl fmt::Display for Mutation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::NewFile(tracking_info) => {
                write!(f, "New file: {:?}", tracking_info.file_path)
            }
            Self::FileRemoved((path, _)) => write!(f, "File removed: {path:?}"),
            Self::FileContentChanged((_, tracking_info)) => {
                write!(f, "File content changed: {:?}", tracking_info.file_path)
            }
            Self::NewFolder(tracking_info) => {
                write!(f, "New folder: {:?}", tracking_info.file_path)
            }
            Self::FolderRemoved((path, _)) => write!(f, "Folder removed: {path:?}"),
        }
    }
}

// Information stored locally to keep track of local changes to files/folders.
// TODO: to make file changes discovery more efficient, and prevent chunking for
// such purposes, add more info like file size and last modified timestamp.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub(super) struct MetadataTrackingInfo {
    pub file_path: PathBuf,
    pub meta_xorname: XorName,
    pub metadata: Metadata,
    pub entry_hash: EntryHash,
}

// Build absolute paths for the different dirs to be used for locally tracking changes
pub(super) fn build_tracking_info_paths(path: &Path) -> Result<(PathBuf, PathBuf, PathBuf)> {
    let files_dir = path.to_path_buf().canonicalize()?;
    let tracking_info_dir = files_dir.join(SAFE_TRACKING_CHANGES_DIR);
    let meta_dir = tracking_info_dir.join(METADATA_CACHE_DIR);
    create_dir_all(&meta_dir)
        .map_err(|err| eyre!("The path provided needs to be a directory: {err}"))?;

    Ok((files_dir, tracking_info_dir, meta_dir))
}

pub(super) fn read_tracking_info_from_disk(
    meta_dir: &Path,
) -> Result<BTreeMap<PathBuf, MetadataTrackingInfo>> {
    let mut curr_tracking_info = BTreeMap::new();
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

        curr_tracking_info.insert(tracking_info.file_path.clone(), tracking_info);
    }

    Ok(curr_tracking_info)
}

// Store tracking info about the root folder in a file to keep track of any changes made
pub(super) fn store_root_folder_tracking_info(
    meta_dir: &Path,
    root_folder_addr: RegisterAddress,
    created: bool,
) -> Result<()> {
    let path = meta_dir.join(ROOT_FOLDER_METADATA_FILENAME);
    let mut meta_file = File::create(path)?;
    meta_file.write_all(&rmp_serde::to_vec(&(root_folder_addr, created))?)?;

    Ok(())
}

// Store the given root seed/SK on disk
// TODO: encrypt the SK with a password
pub(super) fn store_root_sk(dir: &Path, root_sk: &MainSecretKey) -> Result<()> {
    let path = dir.join(RECOVERY_SEED_FILENAME);
    let mut secret_file = File::create(path)?;
    secret_file.write_all(&root_sk.to_bytes())?;

    Ok(())
}

// Read the root seed/SK from disk
// TODO: decrypt the SK with a password
pub(super) fn read_root_sk(dir: &Path) -> Result<MainSecretKey> {
    let path = dir.join(RECOVERY_SEED_FILENAME);
    let bytes = std::fs::read(&path).map_err(|err| {
        eyre!("Error while reading the recovery seed/secret from {path:?}: {err:?}")
    })?;

    let mut buffer = [0u8; SK_SIZE];
    buffer[..SK_SIZE].copy_from_slice(&bytes);
    let sk = MainSecretKey::new(SecretKey::from_bytes(buffer)?);

    Ok(sk)
}

// Read the tracking info about the root folder
pub(super) fn read_root_folder_addr(meta_dir: &Path) -> Result<(RegisterAddress, bool)> {
    let path = meta_dir.join(ROOT_FOLDER_METADATA_FILENAME);
    let bytes = std::fs::read(&path)
        .map_err(|err| eyre!("Error while reading the tracking info from {path:?}: {err:?}"))?;

    Ok(rmp_serde::from_slice(&bytes)?)
}
