// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

/// Chunks
pub mod chunks;
mod errors;

use self::chunks::{Chunk, ChunkAddress};
use chunks::ChunkStorage;
use errors::Result;

/// Operations on data stored to disk.
/// As data the storage struct may be cloned throughoout the node
/// Operations here must be persisted to disk.
// exposed as pub due to benches
#[derive(Clone, Default)]
pub struct DataStorage {
    chunks: ChunkStorage,
}

impl DataStorage {
    /// Set up a new `DataStorage` instance
    pub fn new() -> Self {
        Self {
            chunks: ChunkStorage::default(),
        }
    }

    /// Store data in the local store
    pub async fn store(&self, chunk: &Chunk) -> Result<()> {
        self.chunks.store(chunk).await
    }

    /// Query the local store and return the Chunk
    pub async fn query(&self, addr: &ChunkAddress) -> Result<Chunk> {
        self.chunks.get(addr).await
    }
}
