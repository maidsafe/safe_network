// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::protocol::types::{
    address::ChunkAddress,
    chunk::Chunk,
    error::{Error, Result},
};

use clru::CLruCache;
use std::{num::NonZeroUsize, sync::Arc};
use tokio::sync::RwLock;
use tracing::trace;

const CHUNKS_CACHE_SIZE: usize = 20 * 1024 * 1024;

/// Operations on data chunks.
#[derive(Clone)]
pub(crate) struct ChunkStorage {
    cache: Arc<RwLock<CLruCache<ChunkAddress, Chunk>>>,
}

impl ChunkStorage {
    pub(crate) fn new() -> Self {
        let capacity =
            NonZeroUsize::new(CHUNKS_CACHE_SIZE).expect("Failed to create in-memory Chunk storage");
        Self {
            cache: Arc::new(RwLock::new(CLruCache::new(capacity))),
        }
    }

    // Read chunk from local store
    pub(crate) async fn get(&self, address: &ChunkAddress) -> Result<Chunk> {
        trace!("Getting Chunk: {address:?}");
        if let Some(chunk) = self.cache.read().await.peek(address) {
            Ok(chunk.clone())
        } else {
            Err(Error::ChunkNotFound(*address))
        }
    }

    /// Store a chunk in the local in-memory store unless it is already there
    pub(crate) async fn store(&self, chunk: &Chunk) -> Result<()> {
        let address = chunk.address();
        trace!("About to store Chunk: {address:?}");

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
        trace!("Removing Chunk: {address:?}");
        if self.cache.write().await.pop(address).is_some() {
            Ok(())
        } else {
            Err(Error::ChunkNotFound(*address))
        }
    }
}

impl Default for ChunkStorage {
    fn default() -> Self {
        Self::new()
    }
}
