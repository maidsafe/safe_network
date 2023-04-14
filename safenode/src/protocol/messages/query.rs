// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::spend::SpendQuery;

use crate::protocol::{
    messages::RegisterQuery,
    types::address::{ChunkAddress, DataAddress},
};

use serde::{Deserialize, Serialize};

/// Data queries - retrieving data and inspecting their structure.
///
/// See the [`types`] module documentation for more details of the types supported by the Safe
/// Network, and their semantics.
///
/// [`types`]: crate::protocol::types
#[allow(clippy::large_enum_variant)]
#[derive(Eq, PartialEq, PartialOrd, Clone, Serialize, Deserialize, Debug)]
pub enum Query {
    /// Retrieve a [`Chunk`] at the given address.
    ///
    /// This should eventually lead to a [`GetChunk`] response.
    ///
    /// [`Chunk`]:  crate::protocol::types::chunk::Chunk
    /// [`GetChunk`]: super::QueryResponse::GetChunk
    GetChunk(ChunkAddress),
    /// [`Register`] read operation.
    ///
    /// [`Register`]: crate::protocol::types::register::Register
    Register(RegisterQuery),
    /// [`Spend`] read operation.
    ///
    /// [`Spend`]: super::transfers::SpendQuery.
    Spend(SpendQuery),
}

impl Query {
    /// Used to send a query to the close group of the address.
    pub fn dst(&self) -> DataAddress {
        match self {
            Query::GetChunk(address) => DataAddress::Chunk(*address),
            Query::Register(query) => DataAddress::Register(query.dst()),
            Query::Spend(query) => DataAddress::Spend(query.dst()),
        }
    }
}
