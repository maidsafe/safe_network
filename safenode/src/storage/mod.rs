// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

pub mod chunks;
mod errors;

use chunks::ChunkStorage;
use errors::{Error, Result};
use std::path::{Path, PathBuf};
use tracing::warn;
use walkdir::WalkDir;
use xor_name::XorName;

use self::chunks::{Chunk, ChunkAddress};

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

    // Query the local store and return NodeQueryResponse
    pub async fn query(&self, addr: &ChunkAddress) -> Result<Chunk> {
        self.chunks.get(addr).await
    }

    /// --- System calls ---

    // // Read data from local store
    // pub(crate) async fn get_from_local_store(&self, addr: &ChunkAddress) -> Result<Chunk> {
    //     self.chunks.get_chunk(addr).await
    // }

    #[allow(dead_code)]
    pub(crate) async fn remove(&mut self, addr: &ChunkAddress) -> Result<()> {
        self.chunks.remove_chunk(addr).await
    }

    // /// Retrieve all ReplicatedDataAddresses of stored data
    // pub async fn data_addrs(&self) -> Vec<DataAddress> {
    //     // TODO: Parallelize this below loops
    //     self.chunks
    //         .addrs()
    //         .into_iter()
    //         .map(DataAddress::Bytes)
    //         .chain(
    //             self.registers
    //                 .addrs()
    //                 .await
    //                 .into_iter()
    //                 .map(DataAddress::Register),
    //         )
    //         .collect()
    // }
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

fn list_files_in(path: &Path) -> Vec<PathBuf> {
    if !path.exists() {
        return vec![];
    }

    WalkDir::new(path)
        .into_iter()
        .filter_map(|e| match e {
            Ok(direntry) => Some(direntry),
            Err(err) => {
                warn!("Store: failed to process filesystem entry: {}", err);
                None
            }
        })
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.path().to_path_buf())
        .collect()
}
