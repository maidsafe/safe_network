// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{messages::Nonce, NetworkAddress};
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
    /// Retrieve the cost of storing a record at the given address.
    GetStoreCost(NetworkAddress),
    /// Retrieve a specific record from a specific peer.
    ///
    /// This should eventually lead to a [`GetReplicatedRecord`] response.
    ///
    /// [`GetReplicatedRecord`]: super::QueryResponse::GetReplicatedRecord
    GetReplicatedRecord {
        /// Sender of the query
        requester: NetworkAddress,
        /// Key of the record to be fetched
        key: NetworkAddress,
    },
    /// Get the proof that the chunk with the given NetworkAddress exists with the requested node.
    GetChunkExistenceProof {
        /// The Address of the chunk that we are trying to verify.
        key: NetworkAddress,
        /// The random nonce that the node uses to produce the Proof (i.e., hash(record+nonce))
        nonce: Nonce,
    },
    /// Queries close_group peers whether the target peer is a bad_node
    CheckNodeInProblem(NetworkAddress),
}

impl std::fmt::Display for Query {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Query::GetStoreCost(address) => {
                write!(f, "Query::GetStoreCost({address:?})")
            }
            Query::GetReplicatedRecord { key, requester } => {
                write!(f, "Query::GetStoreCost({requester:?} {key:?})")
            }
            Query::GetChunkExistenceProof { key, nonce } => {
                write!(f, "Query::GetChunkExistenceProof({key:?} {nonce:?})")
            }
            Query::CheckNodeInProblem(address) => {
                write!(f, "Query::CheckNodeInProblem({address:?})")
            }
        }
    }
}
