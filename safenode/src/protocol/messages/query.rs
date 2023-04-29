// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::spend::SpendQuery;

use crate::{
    domain::storage::{ChunkAddress, DataAddress},
    protocol::{messages::RegisterQuery, NetworkKey},
};

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Data queries - retrieving data and inspecting their structure.
///
/// See the [`protocol`] module documentation for more details of the types supported by the Safe
/// Network, and their semantics.
///
/// [`protocol`]: crate::protocol
#[allow(clippy::large_enum_variant)]
#[derive(Eq, PartialEq, PartialOrd, Clone, Serialize, Deserialize, Debug)]
pub enum Query {
    /// Asks the receiving node to send the chunks that the sender
    /// should have, but are not in the provided set of addresses.
    GetMissingData {
        /// The sender of the query.
        /// This is the locality the sender is interested in (i.e. itself),
        /// so the sender asks for the nodes and data around this locality.
        sender: NetworkKey,
        /// The set of addresses that the sender already has.
        existing_data: BTreeSet<ChunkAddress>,
    },
    /// Retrieve a [`Chunk`] at the given address.
    ///
    /// This should eventually lead to a [`GetChunk`] response.
    ///
    /// [`Chunk`]:  crate::domain::storage::Chunk
    /// [`GetChunk`]: super::QueryResponse::GetChunk
    GetChunk(ChunkAddress),
    /// [`Register`] read operation.
    ///
    /// [`Register`]: crate::domain::storage::register::Register
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
            Query::GetMissingData { sender, .. } => DataAddress::Network(sender.clone()),
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
            Query::Spend(query) => {
                write!(f, "Query::Spend({query:?})")
            }
            Query::GetMissingData { sender, .. } => {
                write!(f, "Query::GetMissingData({sender:?})")
            }
        }
    }
}
