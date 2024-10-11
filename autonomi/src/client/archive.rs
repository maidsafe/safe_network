// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use std::{collections::HashMap, path::PathBuf};

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

/// An archive of files that containing file paths, their metadata and the files data addresses
/// Using archives is useful for uploading entire directories to the network, only needing to keep track of a single address.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Archive {
    pub map: HashMap<PathBuf, DataAddr>,
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
