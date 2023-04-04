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
use std::path::{Path, PathBuf};
use xor_name::XorName;

const BIT_TREE_DEPTH: usize = 20;

/// Operations on data stored to disk.
/// As data the storage struct may be cloned throughoout the node
/// Operations here must be persisted to disk.
#[derive(Debug, Clone)]
// exposed as pub due to benches
pub struct DataStorage {
    chunks: ChunkStorage,
}

impl DataStorage {
    /// Set up a new `DataStorage` instance
    pub fn new(path: &Path) -> Self {
        Self {
            chunks: ChunkStorage::new(path),
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

// Helper that returns the prefix tree path of depth BIT_TREE_DEPTH for a given xorname
// Example:
// - with a xorname with starting bits `010001110110....`
// - and a BIT_TREE_DEPTH of `6`
// returns the path `ROOT_PATH/0/1/0/0/0/1`
fn prefix_tree_path(root: &Path, xorname: XorName) -> PathBuf {
    let bin = format!("{xorname:b}");
    let prefix_dir_path: PathBuf = bin.chars().take(BIT_TREE_DEPTH).map(String::from).collect();
    root.join(prefix_dir_path)
}
