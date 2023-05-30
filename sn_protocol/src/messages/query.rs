// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    storage::{ChunkAddress, DbcAddress},
    NetworkAddress,
};

use super::RegisterQuery;

use serde::{Deserialize, Serialize};

/// Data queries - retrieving data and inspecting their structure.
///
/// See the [`protocol`] module documentation for more details of the types supported by the Safe
/// Network, and their semantics.
///
/// [`protocol`]: crate
#[allow(clippy::large_enum_variant)]
#[derive(Eq, PartialEq, PartialOrd, Clone, Serialize, Deserialize, Debug)]
pub enum Query {
    /// Retrieve a [`Chunk`] at the given address.
    ///
    /// This should eventually lead to a [`GetChunk`] response.
    ///
    /// [`Chunk`]:  crate::storage::Chunk
    /// [`GetChunk`]: super::QueryResponse::GetChunk
    GetChunk(ChunkAddress),
    /// [`Register`] read operation.
    ///
    /// [`Register`]: crate::storage::Register
    Register(RegisterQuery),
    /// Retrieve a [`SignedSpend`] at the given address.
    ///
    /// This should eventually lead to a [`GetDbcSpend`] response.
    ///
    /// [`SignedSpend`]: sn_dbc::SignedSpend
    /// [`GetDbcSpend`]: super::QueryResponse::GetDbcSpend
    GetSpend(DbcAddress),
}

impl Query {
    /// Used to send a query to the close group of the address.
    pub fn dst(&self) -> NetworkAddress {
        match self {
            Query::GetChunk(address) => NetworkAddress::from_chunk_address(*address),
            Query::Register(query) => NetworkAddress::from_register_address(query.dst()),
            Query::GetSpend(address) => NetworkAddress::from_dbc_address(*address),
        }
    }
}

impl std::fmt::Display for Query {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Query::GetChunk(address) => {
                write!(f, "Query::GetChunk({address:?})")
            }
            Query::Register(query) => {
                write!(f, "Query::Register({:?})", query.dst()) // more qualification needed
            }
            Query::GetSpend(address) => {
                write!(f, "Query::GetSpend({address:?})")
            }
        }
    }
}
