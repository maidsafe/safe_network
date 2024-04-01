// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use sn_client::{
    protocol::storage::RegisterAddress, registers::EntryHash, transfers::MainSecretKey, FoldersApi,
    Metadata,
};

use aes::Aes256;
use block_modes::{block_padding::Pkcs7, BlockMode, Cbc};
use bls::{SecretKey, SK_SIZE};
use color_eyre::{
    eyre::{bail, eyre},
    Result,
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fmt,
    fs::{create_dir_all, File},
    io::Write,
    path::{Path, PathBuf},
};
use tiny_keccak::{Hasher, Sha3};
use walkdir::WalkDir;
use xor_name::XorName;

// AES used to encrypt/decrypt the cached recovery seed.
type Aes256Cbc = Cbc<Aes256, Pkcs7>;

// AES Initialisation Vector length used.
const IV_LENGTH: usize = 16;

// Length of buffers used for AES encryption/decryption.
const AES_BUFFER_LENGTH: usize = 48;

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

// Store the given root seed/SK on disk, (optionally) encrypted with a password
pub(super) fn store_root_sk(
    dir: &Path,
    root_sk: &MainSecretKey,
    password: Option<&[u8]>,
) -> Result<()> {
    let path = dir.join(RECOVERY_SEED_FILENAME);
    let mut secret_file = File::create(path)?;
    let seed_bytes = root_sk.to_bytes();

    if let Some(pwd) = password {
        // encrypt the SK with the (hashed) password
        let key = encryption_key_from_hashed_password(pwd);

        let pos = seed_bytes.len();
        let mut buffer = [0u8; AES_BUFFER_LENGTH];
        buffer[..pos].copy_from_slice(&seed_bytes);

        // IV is randomly chosen and prefixed it to cipher
        let mut rng = rand::thread_rng();
        let random_iv: [u8; IV_LENGTH] = rng.gen();
        let mut iv_with_cipher = vec![];
        iv_with_cipher.extend(random_iv);

        let cipher = Aes256Cbc::new_from_slices(&key, &random_iv)?;
        let ciphertext = cipher.encrypt(&mut buffer, pos)?;
        iv_with_cipher.extend(ciphertext);

        secret_file.write_all(&iv_with_cipher)?;
    } else {
        secret_file.write_all(&seed_bytes)?;
    }

    Ok(())
}

// Read the root seed/SK from disk, (optionally) decrypting it with a password
pub(super) fn read_root_sk(dir: &Path, password: Option<&[u8]>) -> Result<MainSecretKey> {
    let path = dir.join(RECOVERY_SEED_FILENAME);
    let mut bytes = std::fs::read(&path).map_err(|err| {
        eyre!("Error while reading the recovery seed/secret from {path:?}: {err:?}")
    })?;

    if let Some(pwd) = password {
        // decrypt the SK with the (hashed) password
        if bytes.len() < IV_LENGTH + AES_BUFFER_LENGTH {
            bail!(
                "Not enough bytes found on disk ({}) to decrypt the recovery seed",
                bytes.len()
            );
        }

        // the IV is prefixed
        let mut iv = [0u8; IV_LENGTH];
        iv[..IV_LENGTH].copy_from_slice(&bytes[..IV_LENGTH]);

        let mut buffer = [0u8; AES_BUFFER_LENGTH];
        buffer[..48].copy_from_slice(&bytes[IV_LENGTH..]);

        let key = encryption_key_from_hashed_password(pwd);
        let cipher = Aes256Cbc::new_from_slices(&key, &iv)?;
        bytes = cipher
            .decrypt_vec(&buffer)
            .map_err(|_| eyre!("Failed to decrypt the recovery seed with the provided password"))?;
    }

    if bytes.len() != SK_SIZE {
        bail!(
            "The length of bytes read from disk ({}) doesn't match a recovery seed's length ({SK_SIZE})", bytes.len()
        );
    }
    let mut seed_bytes = [0u8; SK_SIZE];
    seed_bytes[..SK_SIZE].copy_from_slice(&bytes);
    let sk = MainSecretKey::new(SecretKey::from_bytes(seed_bytes)?);

    Ok(sk)
}

fn encryption_key_from_hashed_password(password: &[u8]) -> [u8; 32] {
    let mut key = [0; 32];
    let mut hasher = Sha3::v256();
    hasher.update(password);
    hasher.finalize(&mut key);
    key
}

// Read the tracking info about the root folder
pub(super) fn read_root_folder_addr(meta_dir: &Path) -> Result<(RegisterAddress, bool)> {
    let path = meta_dir.join(ROOT_FOLDER_METADATA_FILENAME);
    let bytes = std::fs::read(&path)
        .map_err(|err| eyre!("Error while reading the tracking info from {path:?}: {err:?}"))?;

    Ok(rmp_serde::from_slice(&bytes)?)
}
