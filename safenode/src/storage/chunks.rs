// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::errors::{Error, Result};
use super::prefix_tree_path;
use async_std::fs::{create_dir_all, read, File};
use bytes::Bytes;
use futures::AsyncWriteExt;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{
    fmt::{self, Display, Formatter},
    io::{self, ErrorKind},
    path::{Path, PathBuf},
};
// use tokio::{
//     fs::{create_dir_all, metadata, read, remove_file, File},
//     io::AsyncWriteExt,
// };
use tracing::{debug, info, trace};
use xor_name::XorName;

const CHUNKS_STORE_DIR_NAME: &str = "chunks";

/// The XorName of the provided `Chunk`
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Debug)]
pub struct ChunkAddress(pub XorName);
impl ChunkAddress {
    /// Returns the name.
    pub fn name(&self) -> &XorName {
        &self.0
    }
}

/// Operations on data chunks.
#[derive(Clone, Debug)]
pub(super) struct ChunkStorage {
    file_store_path: PathBuf,
}

/// Chunk, an immutable chunk of data
#[derive(Hash, Eq, PartialEq, PartialOrd, Ord, Clone, custom_debug::Debug)]
pub struct Chunk {
    /// Network address. Omitted when serialising and
    /// calculated from the `value` when deserialising.
    address: ChunkAddress,
    /// Contained data.
    #[debug(skip)]
    value: Bytes,
}

impl Chunk {
    /// Creates a new instance of `Chunk`.
    pub fn new(value: Bytes) -> Self {
        Self {
            address: ChunkAddress(XorName::from_content(value.as_ref())),
            value,
        }
    }

    /// Returns the value.
    pub fn value(&self) -> &Bytes {
        &self.value
    }

    /// Returns the address.
    pub fn address(&self) -> &ChunkAddress {
        &self.address
    }

    /// Returns the name.
    pub fn name(&self) -> &XorName {
        self.address.name()
    }

    /// Returns size of contained value.
    pub fn payload_size(&self) -> usize {
        self.value.len()
    }

    /// Returns size of this chunk after serialisation.
    pub fn serialised_size(&self) -> usize {
        self.value.len()
    }
}

impl Serialize for Chunk {
    fn serialize<S: Serializer>(&self, serialiser: S) -> Result<S::Ok, S::Error> {
        // Address is omitted since it's derived from value
        self.value.serialize(serialiser)
    }
}

impl<'de> Deserialize<'de> for Chunk {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Deserialize::deserialize(deserializer)?;
        Ok(Self::new(value))
    }
}

impl ChunkStorage {
    /// Creates a new `ChunkStorage` at the specified root location
    ///
    /// If the location specified already contains a `ChunkStorage`, it is simply used
    ///
    /// Used space of the dir is tracked
    pub(super) fn new(path: &Path) -> Self {
        Self {
            file_store_path: path.join(CHUNKS_STORE_DIR_NAME),
        }
    }

    fn chunk_addr_to_filepath(&self, addr: &ChunkAddress) -> Result<PathBuf> {
        let xorname = *addr.name();
        let path = prefix_tree_path(&self.file_store_path, xorname);
        let filename = hex::encode(xorname);
        Ok(path.join(filename))
    }

    pub(super) async fn get_chunk(&self, address: &ChunkAddress) -> Result<Chunk> {
        trace!("Getting chunk {:?}", address);

        let file_path = self.chunk_addr_to_filepath(address)?;
        match read(file_path).await {
            Ok(bytes) => {
                let chunk = Chunk::new(Bytes::from(bytes));
                if chunk.address() != address {
                    // This can happen if the content read is empty, or incomplete,
                    // possibly due to an issue with the OS synchronising to disk,
                    // resulting in a mismatch with recreated address of the Chunk.
                    Err(Error::ChunkNotFound(*address.name()))
                } else {
                    Ok(chunk)
                }
            }
            Err(io_error @ io::Error { .. }) if io_error.kind() == ErrorKind::NotFound => {
                Err(Error::ChunkNotFound(*address.name()))
            }
            Err(other) => Err(other.into()),
        }
    }

    // Read chunk from local store and return NodeQueryResponse
    pub(super) async fn get(&self, address: &ChunkAddress) -> Result<Chunk> {
        self.get_chunk(address).await
    }

    /// Store a chunk in the local disk store unless it is already there
    pub(super) async fn store(&self, chunk: &Chunk) -> Result<()> {
        let addr = chunk.address();
        let filepath = self.chunk_addr_to_filepath(addr)?;

        if filepath.exists() {
            info!(
                "{}: Chunk data already exists, not storing: {:?}",
                self, addr
            );
            // Nothing more to do here
            return Ok(());
        }

        // Store the data on disk
        if let Some(dirs) = filepath.parent() {
            create_dir_all(dirs).await?;
        }

        let mut file = File::create(filepath).await?;

        file.write_all(chunk.value()).await?;
        // Let's sync up OS data to disk to reduce the chances of
        // concurrent reading failing by reading an empty/incomplete file
        file.sync_data().await?;

        Ok(())
    }
}

impl Display for ChunkStorage {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(formatter, "ChunkStorage")
    }
}
