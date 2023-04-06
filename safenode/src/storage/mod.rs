// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::protocol::{
    messages::{Query, QueryResponse, ReplicatedData},
    types::{address::ChunkAddress, chunk::Chunk, errors::Result, register::User},
};

mod chunks;
mod register_store;
mod registers;

use chunks::ChunkStorage;
use registers::RegisterStorage;
use tracing::debug;

/// Operations on data stored to disk.
/// As data the storage struct may be cloned throughoout the node
/// Operations here must be persisted to disk.
// Exposed as pub due to benches.
#[derive(Clone, Default)]
pub struct DataStorage {
    chunks: ChunkStorage,
    registers: RegisterStorage,
}

impl DataStorage {
    /// Set up a new `DataStorage` instance
    pub fn new() -> Self {
        Self {
            chunks: ChunkStorage::default(),
            registers: RegisterStorage::new(),
        }
    }

    /// Store Chunk in the local store
    pub async fn store_chunk(&self, chunk: &Chunk) -> Result<()> {
        self.chunks.store(chunk).await
    }

    /// Query the local store and return the Chunk
    pub async fn query_chunk(&self, addr: &ChunkAddress) -> Result<Chunk> {
        self.chunks.get(addr).await
    }

    /// Store data in the local store
    pub async fn store(&self, data: &ReplicatedData) -> Result<()> {
        debug!("Replicating {data:?}");
        match data {
            ReplicatedData::Chunk(chunk) => self.chunks.store(chunk).await,
            ReplicatedData::RegisterLog(data) => self.registers.update(data).await,
            ReplicatedData::RegisterWrite(cmd) => self.registers.write(cmd).await,
        }
    }

    /// Query the local store and return QueryResponse
    pub async fn query(&self, query: &Query, requester: User) -> QueryResponse {
        match query {
            Query::GetChunk(addr) => QueryResponse::GetChunk(self.chunks.get(addr).await),
            Query::Register(read) => self.registers.read(read, requester).await,
            Query::GetDbc(_) => todo!(),
        }
    }
}
