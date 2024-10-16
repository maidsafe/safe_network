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
    time::SystemTime,
};

use super::{
    data::DataAddr,
    data::{GetError, PutError},
    Client,
};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use sn_evm::EvmWallet;
use xor_name::XorName;

/// The address of an archive on the network. Points to an [`Archive`].
pub type ArchiveAddr = XorName;

use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum ArchiveError {
    #[error("File not found in archive: {0}")]
    FileNotFound(PathBuf),
}

/// An archive of files that containing file paths, their metadata and the files data addresses
/// Using archives is useful for uploading entire directories to the network, only needing to keep track of a single address.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Archive {
    map: HashMap<PathBuf, (DataAddr, Metadata)>,
}

/// Metadata for a file in an archive
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub created: SystemTime,
}

impl Metadata {
    /// Create a new metadata struct
    pub fn new() -> Self {
        Self {
            created: SystemTime::now(),
        }
    }
}

impl Default for Metadata {
    fn default() -> Self {
        Self::new()
    }
}

impl Archive {
    /// Deserialize from bytes.
    pub fn from_bytes(data: Bytes) -> Result<Archive, rmp_serde::decode::Error> {
        let root: Archive = rmp_serde::from_slice(&data[..])?;

        Ok(root)
    }

    /// Serialize to bytes.
    pub fn into_bytes(&self) -> Result<Bytes, rmp_serde::encode::Error> {
        let root_serialized = rmp_serde::to_vec(&self)?;
        let root_serialized = Bytes::from(root_serialized);

        Ok(root_serialized)
    }

    /// Rename a file in an archive
    /// Note that this does not upload the archive to the network
    pub fn rename_file(&mut self, old_path: &Path, new_path: &Path) -> Result<(), ArchiveError> {
        let data_addr = self
            .map
            .remove(old_path)
            .ok_or(ArchiveError::FileNotFound(old_path.to_path_buf()))?;
        self.map.insert(new_path.to_path_buf(), data_addr);
        Ok(())
    }

    /// Create a new emtpy local archive
    /// Note that this does not upload the archive to the network
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    /// Add a file to a local archive
    /// Note that this does not upload the archive to the network
    pub fn add_file(&mut self, path: PathBuf, data_addr: DataAddr, meta: Metadata) {
        self.map.insert(path, (data_addr, meta));
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
}

impl Default for Archive {
    fn default() -> Self {
        Self::new()
    }
}

impl Client {
    /// Fetch an archive from the network
    pub async fn archive_get(&self, addr: ArchiveAddr) -> Result<Archive, GetError> {
        let data = self.data_get(addr).await?;
        Ok(Archive::from_bytes(data)?)
    }

    /// Upload an archive to the network
    pub async fn archive_put(
        &self,
        archive: Archive,
        wallet: &EvmWallet,
    ) -> Result<ArchiveAddr, PutError> {
        let bytes = archive
            .into_bytes()
            .map_err(|e| PutError::Serialization(format!("Failed to serialize archive: {e:?}")))?;
        self.data_put(bytes, wallet).await
    }
}
