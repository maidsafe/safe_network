// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use ant_networking::target_arch::{Duration, SystemTime, UNIX_EPOCH};

use ant_evm::{AttoTokens, EvmWallet};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use xor_name::XorName;

use super::archive::Metadata;
use crate::{
    client::{
        data::{CostError, DataAddr, GetError, PutError},
        files::archive::RenameError,
    },
    Client,
};

/// The address of a public archive on the network. Points to an [`PublicArchive`].
pub type ArchiveAddr = XorName;

/// Public variant of [`crate::client::files::archive::PrivateArchive`]. Differs in that data maps of files are uploaded
/// to the network, of which the addresses are stored in this archive.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct PublicArchive {
    map: HashMap<PathBuf, (DataAddr, Metadata)>,
}

impl PublicArchive {
    /// Create a new emtpy local archive
    /// Note that this does not upload the archive to the network
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    /// Rename a file in an archive
    /// Note that this does not upload the archive to the network
    pub fn rename_file(&mut self, old_path: &Path, new_path: &Path) -> Result<(), RenameError> {
        let (data_addr, mut meta) = self
            .map
            .remove(old_path)
            .ok_or(RenameError::FileNotFound(old_path.to_path_buf()))?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_secs();
        meta.modified = now;
        self.map.insert(new_path.to_path_buf(), (data_addr, meta));
        debug!("Renamed file successfully in the archive, old path: {old_path:?} new_path: {new_path:?}");
        Ok(())
    }

    /// Add a file to a local archive
    /// Note that this does not upload the archive to the network
    pub fn add_file(&mut self, path: PathBuf, data_addr: DataAddr, meta: Metadata) {
        self.map.insert(path.clone(), (data_addr, meta));
        debug!("Added a new file to the archive, path: {:?}", path);
    }

    /// List all files in the archive
    pub fn files(&self) -> Vec<(PathBuf, Metadata)> {
        self.map
            .iter()
            .map(|(path, (_, meta))| (path.clone(), meta.clone()))
            .collect()
    }

    /// List all data addresses of the files in the archive
    pub fn addresses(&self) -> Vec<DataAddr> {
        self.map.values().map(|(addr, _)| *addr).collect()
    }

    /// Iterate over the archive items
    /// Returns an iterator over (PathBuf, DataAddr, Metadata)
    pub fn iter(&self) -> impl Iterator<Item = (&PathBuf, &DataAddr, &Metadata)> {
        self.map
            .iter()
            .map(|(path, (addr, meta))| (path, addr, meta))
    }

    /// Get the underlying map
    pub fn map(&self) -> &HashMap<PathBuf, (DataAddr, Metadata)> {
        &self.map
    }

    /// Deserialize from bytes.
    pub fn from_bytes(data: Bytes) -> Result<PublicArchive, rmp_serde::decode::Error> {
        let root: PublicArchive = rmp_serde::from_slice(&data[..])?;

        Ok(root)
    }

    /// Serialize to bytes.
    pub fn into_bytes(&self) -> Result<Bytes, rmp_serde::encode::Error> {
        let root_serialized = rmp_serde::to_vec(&self)?;
        let root_serialized = Bytes::from(root_serialized);

        Ok(root_serialized)
    }
}

impl Client {
    /// Fetch an archive from the network
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use autonomi::{Client, client::files::archive_public::ArchiveAddr};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = Client::init().await?;
    /// let archive = client.archive_get_public(ArchiveAddr::random(&mut rand::thread_rng())).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn archive_get_public(&self, addr: ArchiveAddr) -> Result<PublicArchive, GetError> {
        let data = self.data_get_public(addr).await?;
        Ok(PublicArchive::from_bytes(data)?)
    }

    /// Upload an archive to the network
    ///
    /// # Example
    ///
    /// Create simple archive containing `file.txt` pointing to random XOR name.
    ///
    /// ```no_run
    /// # use autonomi::{Client, client::{data::DataAddr, files::{archive::Metadata, archive_public::{PublicArchive, ArchiveAddr}}}};
    /// # use std::path::PathBuf;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = Client::init().await?;
    /// # let wallet = todo!();
    /// let mut archive = PublicArchive::new();
    /// archive.add_file(PathBuf::from("file.txt"), DataAddr::random(&mut rand::thread_rng()), Metadata::new_with_size(0));
    /// let address = client.archive_put_public(archive, &wallet).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn archive_put_public(
        &self,
        archive: PublicArchive,
        wallet: &EvmWallet,
    ) -> Result<ArchiveAddr, PutError> {
        let bytes = archive
            .into_bytes()
            .map_err(|e| PutError::Serialization(format!("Failed to serialize archive: {e:?}")))?;
        let result = self.data_put_public(bytes, wallet.into()).await;
        debug!("Uploaded archive {archive:?} to the network and the address is {result:?}");
        result
    }

    /// Get the cost to upload an archive
    pub async fn archive_cost(&self, archive: PublicArchive) -> Result<AttoTokens, CostError> {
        let bytes = archive
            .into_bytes()
            .map_err(|e| CostError::Serialization(format!("Failed to serialize archive: {e:?}")))?;
        let result = self.data_cost(bytes).await;
        debug!("Calculated the cost to upload archive {archive:?} is {result:?}");
        result
    }
}
