// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod chunks;
mod register_store;
mod registers;
mod spends;

use crate::protocol::{
    messages::{Cmd, CmdResponse, Query, QueryResponse, RegisterCmd},
    types::register::User,
};
use chunks::ChunkStorage;
use registers::RegisterStorage;
use tracing::debug;

use self::spends::SpendStorage;

/// Operations on data stored to disk.
/// As data the storage struct may be cloned throughout the node
/// Operations here must be persisted to disk.
// Exposed as pub due to benches.
#[derive(Clone, Default)]
pub struct DataStorage {
    chunks: ChunkStorage,
    registers: RegisterStorage,
    spends: SpendStorage,
}

impl DataStorage {
    /// Set up a new `DataStorage` instance
    pub fn new() -> Self {
        Self {
            chunks: ChunkStorage::default(),
            registers: RegisterStorage::new(),
            spends: SpendStorage::default(),
        }
    }

    /// Store data in the local store and return `CmdResponse`
    pub async fn write(&self, cmd: &Cmd) -> CmdResponse {
        debug!("Storage write: {cmd:?}");
        match cmd {
            Cmd::StoreChunk(chunk) => CmdResponse::StoreChunk(self.chunks.store(chunk).await),
            Cmd::Register(cmd) => {
                let result = self.registers.write(cmd).await;
                match cmd {
                    RegisterCmd::Create(_) => CmdResponse::CreateRegister(result),
                    RegisterCmd::Edit(_) => CmdResponse::EditRegister(result),
                }
            }
            Cmd::Dbc(spend) => CmdResponse::Spend(self.spends.try_add(spend.signed_spend()).await),
        }
    }

    /// Query the local store and return `QueryResponse`
    pub async fn read(&self, query: &Query, requester: User) -> QueryResponse {
        debug!("Storage read: {query:?}");
        match query {
            Query::GetChunk(addr) => QueryResponse::GetChunk(self.chunks.get(addr).await),
            Query::Register(read) => self.registers.read(read, requester).await,
            Query::GetDbcSpend(address) => {
                QueryResponse::GetDbcSpend(self.spends.get(address).await)
            }
        }
    }
}
