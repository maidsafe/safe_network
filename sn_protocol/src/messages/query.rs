// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    storage::{ChunkAddress, DbcAddress, RegisterAddress},
    NetworkAddress,
};

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
    /// Retrieve a [`SignedRegister`] at the given address.
    ///
    /// This should eventually lead to a [`GetRegister`] response.
    ///
    /// [`SignedRegister`]: sn_registers::SignedRegister
    /// [`GetRegister`]: super::QueryResponse::GetRegister
    GetRegister(RegisterAddress),
    /// Retrieve a [`SignedSpend`] at the given address.
    ///
    /// This should eventually lead to a [`GetDbcSpend`] response.
    ///
    /// [`SignedSpend`]: sn_dbc::SignedSpend
    /// [`GetDbcSpend`]: super::QueryResponse::GetDbcSpend
    GetSpend(DbcAddress),
    /// Retrieve a [`ReplicatedData`] at the given address.
    ///
    /// This should eventually lead to a [`GetReplicatedData`] response.
    ///
    /// [`ReplicatedData`]:  crate::messages::ReplicatedData
    /// [`GetReplicatedData`]: super::QueryResponse::GetReplicatedData
    GetReplicatedData {
        /// Sender of the query
        requester: NetworkAddress,
        /// Address of the data to be fetched
        address: NetworkAddress,
    },
}

impl Query {
    /// Used to send a query to the close group of the address.
    pub fn dst(&self) -> NetworkAddress {
        match self {
            Query::GetChunk(address) => NetworkAddress::from_chunk_address(*address),
            Query::GetRegister(address) => NetworkAddress::from_register_address(*address),
            Query::GetSpend(address) => NetworkAddress::from_dbc_address(*address),
            Query::GetReplicatedData { address, .. } => address.clone(),
        }
    }
}

impl std::fmt::Display for Query {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Query::GetChunk(address) => {
                write!(f, "Query::GetChunk({address:?})")
            }
            Query::GetRegister(address) => {
                write!(f, "Query::GetRegister({address:?})")
            }
            Query::GetSpend(address) => {
                write!(f, "Query::GetSpend({address:?})")
            }
            Query::GetReplicatedData { requester, address } => {
                write!(
                    f,
                    "Query::GetReplicatedData({requester:?} querying {address:?})"
                )
            }
        }
    }
}
