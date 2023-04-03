// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::errors::{Error, Result};

use bytes::Bytes;
use clru::CLruCache;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{num::NonZeroUsize, sync::Arc};
use tokio::sync::RwLock;
use tracing::{debug, trace};
use xor_name::XorName;

const CHUNKS_CACHE_SIZE: usize = 20 * 1024 * 1024;

/// Address of a Chunk
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Debug)]
pub struct ChunkAddress(XorName);
impl ChunkAddress {
    /// Creates a new ChunkAddress.
    pub fn new(xor_name: XorName) -> Self {
        Self(xor_name)
    }

    /// Returns the name.
    pub fn name(&self) -> &XorName {
        &self.0
    }
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

/// Operations on data chunks.
#[derive(Clone)]
pub(super) struct ChunkStorage {
    cache: Arc<RwLock<CLruCache<ChunkAddress, Chunk>>>,
}

impl Default for ChunkStorage {
    fn default() -> Self {
        let capacity =
            NonZeroUsize::new(CHUNKS_CACHE_SIZE).expect("Failed to create in-memory Chunk storage");
        Self {
            cache: Arc::new(RwLock::new(CLruCache::new(capacity))),
        }
    }
}

impl ChunkStorage {
    #[allow(dead_code)]
    pub(super) async fn addrs(&self) -> Vec<ChunkAddress> {
        self.cache
            .read()
            .await
            .iter()
            .map(|(addr, _)| *addr)
            .collect()
    }

    #[allow(dead_code)]
    pub(super) async fn remove_chunk(&self, address: &ChunkAddress) -> Result<()> {
        trace!("Removing chunk: {address:?}");
        if self.cache.write().await.pop(address).is_some() {
            Ok(())
        } else {
            Err(Error::ChunkNotFound(*address.name()))
        }
    }

    // Read chunk from local store
    pub(super) async fn get(&self, address: &ChunkAddress) -> Result<Chunk> {
        trace!("Getting chunk: {address:?}");
        if let Some(chunk) = self.cache.read().await.peek(address) {
            Ok(chunk.clone())
        } else {
            Err(Error::ChunkNotFound(*address.name()))
        }
    }

    /// Store a chunk in the local in-memory store unless it is already there
    pub(super) async fn store(&self, chunk: &Chunk) -> Result<()> {
        let address = chunk.address();
        trace!("About to store chunk: {address:?}");

        let _ = self.cache.write().await.try_put_or_modify(
            *address,
            |addr, _| {
                trace!("Chunk successfully stored: {addr:?}");
                Ok::<Chunk, Error>(chunk.clone())
            },
            |addr, _, _| {
                trace!("Chunk data already exists in cache, not storing: {addr:?}");
                Ok(())
            },
            (),
        )?;

        Ok(())
    }
}
