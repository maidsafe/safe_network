// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{error::Result, Client, ClientRegister, WalletClient};

use sn_protocol::{
    storage::{ChunkAddress, RegisterAddress},
    NetworkAddress,
};
use sn_transfers::{HotWallet, Payment};
use xor_name::XorName;

use libp2p::PeerId;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    ffi::OsString,
    path::{Path, PathBuf},
};

/// Folder Entry representing either a file or subfolder.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum FolderEntry {
    File(ChunkAddress),
    Folder(RegisterAddress),
}

/// Folders APIs.
#[derive(Clone)]
pub struct FoldersApi {
    client: Client,
    wallet_dir: PathBuf,
    register: ClientRegister,
}

impl FoldersApi {
    /// Create FoldersApi instance.
    pub fn new(client: Client, wallet_dir: &Path) -> Self {
        let mut rng = rand::thread_rng();
        let register = ClientRegister::create(client.clone(), XorName::random(&mut rng));
        Self {
            client,
            wallet_dir: wallet_dir.to_path_buf(),
            register,
        }
    }

    /// Return the address of the Folder (Register address) on the network
    pub fn address(&self) -> &RegisterAddress {
        self.register.address()
    }

    /// Return the address of the Folder (Register address) as a NetworkAddress
    pub fn as_net_addr(&self) -> NetworkAddress {
        NetworkAddress::RegisterAddress(*self.address())
    }

    /// Create a new WalletClient from the directory set.
    pub fn wallet(&self) -> Result<WalletClient> {
        let path = self.wallet_dir.as_path();
        let wallet = HotWallet::load_from(path)?;

        Ok(WalletClient::new(self.client.clone(), wallet))
    }

    /// Add provided file as entry of this Folder (locally).
    pub fn add_file(&mut self, name: OsString, address: ChunkAddress) -> Result<()> {
        self.add_entry(name, FolderEntry::File(address))
    }

    /// Add subfolder as entry of this Folder (locally).
    pub fn add_folder(&mut self, name: OsString, address: RegisterAddress) -> Result<()> {
        self.add_entry(name, FolderEntry::Folder(address))
    }

    /// Sync local Folder with the network.
    pub async fn sync(
        &mut self,
        verify_store: bool,
        payment_info: Option<(Payment, PeerId)>,
    ) -> Result<RegisterAddress> {
        let mut wallet_client = self.wallet()?;
        self.register
            .sync(&mut wallet_client, verify_store, payment_info)
            .await?;

        Ok(*self.register.address())
    }

    /// Download a copy of the Folder from the network.
    pub async fn retrieve(
        client: Client,
        wallet_dir: &Path,
        address: RegisterAddress,
    ) -> Result<Self> {
        let register = ClientRegister::retrieve(client.clone(), address).await?;
        Ok(Self {
            client,
            wallet_dir: wallet_dir.to_path_buf(),
            register,
        })
    }

    /// Returns the list of entries of this Folder
    pub fn entries(&self) -> Result<Vec<(OsString, FolderEntry)>> {
        let mut entries = vec![];
        for (_, entry) in self.register.read() {
            let (name, folder_entry): (String, FolderEntry) = rmp_serde::from_slice(&entry)?;
            entries.push((name.into(), folder_entry));
        }
        Ok(entries)
    }

    // Private helpers

    // Add the given entry to the underlying Register
    fn add_entry(&mut self, name: OsString, entry: FolderEntry) -> Result<()> {
        // TODO: conversion to String will be removed when metadata is moved out of the Register
        let name = name.into_string().unwrap_or("unknown".to_string());
        let entry = (name, entry);
        self.register
            .write_atop(&rmp_serde::to_vec(&entry)?, &BTreeSet::default())?;
        Ok(())
    }
}
