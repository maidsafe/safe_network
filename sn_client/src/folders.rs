// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{error::Result, Client, ClientRegister, WalletClient};
use crate::{acc_packet::load_account_wallet_or_create_with_mnemonic, Error, FilesApi, UploadCfg};
use bls::{Ciphertext, PublicKey};
use bytes::{BufMut, BytesMut};
use self_encryption::MAX_CHUNK_SIZE;
use serde::{Deserialize, Serialize};
use sn_protocol::{
    storage::{Chunk, ChunkAddress, RegisterAddress},
    NetworkAddress,
};
use sn_registers::{Entry, EntryHash};

use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    path::{Path, PathBuf},
};
use xor_name::{XorName, XOR_NAME_LEN};

/// Folder Entry representing either a file or subfolder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum FolderEntry {
    File(Chunk),
    Folder(RegisterAddress),
}

/// Metadata to be stored on a Chunk, linked from and belonging to Registers' entries.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Metadata {
    pub name: String,
    pub content: FolderEntry,
}

// This is the entry value used in Folders to mark a removed file/folder.
const REMOVED_ENTRY_MARK: XorName = XorName([0; XOR_NAME_LEN]);

/// Folders APIs.
#[derive(Clone)]
pub struct FoldersApi {
    client: Client,
    wallet_dir: PathBuf,
    register: ClientRegister,
    files_api: FilesApi,
    // Cache of metadata chunks. We keep the Chunk itself till we upload it to the network.
    metadata: BTreeMap<XorName, (Metadata, Option<Chunk>)>,
}

impl FoldersApi {
    /// Create FoldersApi instance.
    pub fn new(
        client: Client,
        wallet_dir: &Path,
        address: Option<RegisterAddress>,
    ) -> Result<Self> {
        let register = if let Some(addr) = address {
            ClientRegister::create_with_addr(client.clone(), addr)
        } else {
            let mut rng = rand::thread_rng();
            ClientRegister::create(client.clone(), XorName::random(&mut rng))
        };

        Self::create(client, wallet_dir, register)
    }

    /// Clones the register instance. Any change made to one instance will not be reflected on the other register.
    pub fn register(&self) -> ClientRegister {
        self.register.clone()
    }

    /// Return the address of the Folder (Register address) on the network
    pub fn address(&self) -> &RegisterAddress {
        self.register.address()
    }

    /// Return the address of the Folder (Register address) as a NetworkAddress
    pub fn as_net_addr(&self) -> NetworkAddress {
        NetworkAddress::RegisterAddress(*self.address())
    }

    /// Return the list of metadata chunks addresses that need to be payed for in order to be
    /// able to then store all data on the network upon calling `sync` method.
    #[allow(clippy::mutable_key_type)]
    pub fn meta_addrs_to_pay(&self) -> BTreeSet<NetworkAddress> {
        self.metadata
            .iter()
            .filter_map(|(meta_xorname, (_, chunk))| {
                chunk
                    .as_ref()
                    .map(|_| NetworkAddress::ChunkAddress(ChunkAddress::new(*meta_xorname)))
            })
            .collect()
    }

    /// Return the list of metadata chunks.
    #[allow(clippy::mutable_key_type)]
    pub fn meta_chunks(&self) -> BTreeSet<Chunk> {
        self.metadata
            .iter()
            .filter_map(|(_, (_, chunk))| chunk.clone())
            .collect()
    }

    /// Create a new WalletClient from the directory set.
    pub fn wallet(&self) -> Result<WalletClient> {
        let wallet = load_account_wallet_or_create_with_mnemonic(&self.wallet_dir, None)?;
        Ok(WalletClient::new(self.client.clone(), wallet))
    }

    /// Add provided file as entry of this Folder (locally).
    /// The new file's metadata chunk will be encrypted if a key has been provided.
    pub fn add_file(
        &mut self,
        file_name: OsString,
        data_map_chunk: Chunk,
        encryption_pk: Option<PublicKey>,
    ) -> Result<(EntryHash, XorName, Metadata)> {
        // create metadata Chunk for this entry
        let metadata = Metadata {
            name: file_name.to_str().unwrap_or("unknown").to_string(),
            content: FolderEntry::File(data_map_chunk),
        };

        self.add_entry(metadata, &BTreeSet::default(), encryption_pk)
    }

    /// Add subfolder as entry of this Folder (locally).
    /// The new folder's metadata chunk will be encrypted if a key has been provided.
    pub fn add_folder(
        &mut self,
        folder_name: OsString,
        address: RegisterAddress,
        encryption_pk: Option<PublicKey>,
    ) -> Result<(EntryHash, XorName, Metadata)> {
        // create metadata Chunk for this entry
        let metadata = Metadata {
            name: folder_name.to_str().unwrap_or("unknown").to_string(),
            content: FolderEntry::Folder(address),
        };

        self.add_entry(metadata, &BTreeSet::default(), encryption_pk)
    }

    /// Replace an existing file with the provided one (locally).
    /// The new file's metadata chunk will be encrypted if a key has been provided.
    pub fn replace_file(
        &mut self,
        existing_entry: EntryHash,
        file_name: OsString,
        data_map_chunk: Chunk,
        encryption_pk: Option<PublicKey>,
    ) -> Result<(EntryHash, XorName, Metadata)> {
        // create metadata Chunk for this entry
        let metadata = Metadata {
            name: file_name.to_str().unwrap_or("unknown").to_string(),
            content: FolderEntry::File(data_map_chunk),
        };

        self.add_entry(
            metadata,
            &vec![existing_entry].into_iter().collect(),
            encryption_pk,
        )
    }

    /// Remove a file/folder item from this Folder (locally).
    pub fn remove_item(&mut self, existing_entry: EntryHash) -> Result<()> {
        let _ = self.register.write_atop(
            &REMOVED_ENTRY_MARK,
            &vec![existing_entry].into_iter().collect(),
        )?;
        Ok(())
    }

    /// Sync local Folder with the network.
    pub async fn sync(&mut self, upload_cfg: UploadCfg) -> Result<()> {
        let mut wallet_client = self.wallet()?;

        // First upload any newly created metadata chunk
        for (_, meta_chunk) in self.metadata.values_mut() {
            if let Some(chunk) = meta_chunk.take() {
                self.files_api
                    .get_local_payment_and_upload_chunk(
                        chunk.clone(),
                        upload_cfg.verify_store,
                        Some(upload_cfg.retry_strategy),
                    )
                    .await?;
            }
        }

        let payment_info = wallet_client.get_recent_payment_for_addr(&self.as_net_addr())?;

        self.register
            .sync(
                &mut wallet_client,
                upload_cfg.verify_store,
                Some(payment_info),
            )
            .await?;

        Ok(())
    }

    /// Download a copy of the Folder from the network.
    pub async fn retrieve(
        client: Client,
        wallet_dir: &Path,
        address: RegisterAddress,
    ) -> Result<Self> {
        let register = ClientRegister::retrieve(client.clone(), address).await?;
        Self::create(client, wallet_dir, register)
    }

    /// Returns true if there is a file/folder which matches the given entry hash
    pub fn contains(&self, entry_hash: &EntryHash) -> bool {
        self.register
            .read()
            .iter()
            .any(|(hash, _)| hash == entry_hash)
    }

    /// Find file/folder in this Folder by its name, returning metadata chunk xorname and metadata itself.
    pub fn find_by_name(&self, name: &str) -> Option<(&XorName, &Metadata)> {
        // let's get the list of metadata xornames of non-removed entries
        let non_removed_items: BTreeSet<XorName> = self
            .register
            .read()
            .iter()
            .map(|(_, meta_xorname_entry)| xorname_from_entry(meta_xorname_entry))
            .collect();

        self.metadata
            .iter()
            .find_map(|(meta_xorname, (metadata, _))| {
                if metadata.name == name && non_removed_items.contains(meta_xorname) {
                    Some((meta_xorname, metadata))
                } else {
                    None
                }
            })
    }

    /// Returns the list of entries of this Folder, including their entry hash,
    /// metadata chunk xorname, and metadata itself.
    pub async fn entries(&mut self) -> Result<BTreeMap<EntryHash, (XorName, Metadata)>> {
        let mut entries = BTreeMap::new();
        for (entry_hash, entry) in self.register.read() {
            let meta_xorname = xorname_from_entry(&entry);
            if meta_xorname == REMOVED_ENTRY_MARK {
                continue;
            }

            let metadata = match self.metadata.get(&meta_xorname) {
                Some((metadata, _)) => metadata.clone(),
                None => {
                    // retrieve metadata Chunk from network
                    let chunk = self
                        .client
                        .get_chunk(ChunkAddress::new(meta_xorname), false, None)
                        .await?;

                    // let's first assume it's unencrypted
                    let metadata: Metadata = match rmp_serde::from_slice(chunk.value()) {
                        Ok(metadata) => metadata,
                        Err(err) => {
                            // let's try to decrypt it then
                            let cipher = Ciphertext::from_bytes(chunk.value()).map_err(|_| err)?;
                            let data = self
                                .client
                                .signer()
                                .decrypt(&cipher)
                                .ok_or(Error::FolderEntryDecryption(entry_hash))?;

                            // if this fails, it's either the wrong key or unexpected data
                            rmp_serde::from_slice(&data)
                                .map_err(|_| Error::FolderEntryDecryption(entry_hash))?
                        }
                    };
                    self.metadata.insert(meta_xorname, (metadata.clone(), None));
                    metadata
                }
            };
            entries.insert(entry_hash, (meta_xorname, metadata));
        }
        Ok(entries)
    }

    // Private helpers

    // Create a new FoldersApi instance with given register.
    fn create(client: Client, wallet_dir: &Path, register: ClientRegister) -> Result<Self> {
        let files_api = FilesApi::new(client.clone(), wallet_dir.to_path_buf());

        Ok(Self {
            client,
            wallet_dir: wallet_dir.to_path_buf(),
            register,
            files_api,
            metadata: BTreeMap::new(),
        })
    }

    // Add the given entry to the underlying Register as well as creating the metadata Chunk.
    // If an encryption key is given, the metadata chunk will be encrpyted with it.
    fn add_entry(
        &mut self,
        metadata: Metadata,
        children: &BTreeSet<EntryHash>,
        encryption_pk: Option<PublicKey>,
    ) -> Result<(EntryHash, XorName, Metadata)> {
        let mut bytes = BytesMut::with_capacity(MAX_CHUNK_SIZE);
        let serialised_metadata = rmp_serde::to_vec(&metadata)?;
        if let Some(pk) = encryption_pk {
            bytes.put(
                pk.encrypt(serialised_metadata.as_slice())
                    .to_bytes()
                    .as_slice(),
            );
        } else {
            bytes.put(serialised_metadata.as_slice());
        }
        let meta_chunk = Chunk::new(bytes.freeze());
        let meta_xorname = *meta_chunk.name();

        self.metadata
            .insert(meta_xorname, (metadata.clone(), Some(meta_chunk)));
        let entry_hash = self.register.write_atop(&meta_xorname, children)?;

        Ok((entry_hash, meta_xorname, metadata))
    }
}

// Helper to convert a Register/Folder entry into a XorName
fn xorname_from_entry(entry: &Entry) -> XorName {
    let mut xorname = [0; XOR_NAME_LEN];
    xorname.copy_from_slice(entry);
    XorName(xorname)
}
