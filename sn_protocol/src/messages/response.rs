// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{error::Result, NetworkAddress};

use super::ChunkProof;
use bytes::Bytes;
use core::fmt;
use serde::{Deserialize, Serialize};
use sn_transfers::{MainPubkey, PaymentQuote};
use std::fmt::Debug;

/// The response to a query, containing the query result.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueryResponse {
    // ===== GetStoreCost =====
    //
    /// Response to [`GetStoreCost`]
    ///
    /// [`GetStoreCost`]: crate::messages::Query::GetStoreCost
    GetStoreCost {
        /// The store cost quote for storing the next record.
        quote: Result<PaymentQuote>,
        /// The cash_note MainPubkey to pay this node's store cost to.
        payment_address: MainPubkey,
        /// Node's Peer Address
        peer_address: NetworkAddress,
    },
    CheckNodeInProblem {
        /// Address of the peer that queried
        reporter_address: NetworkAddress,
        /// Address of the target to be queried
        target_address: NetworkAddress,
        /// Status flag indicating whether the target is in trouble
        is_in_trouble: bool,
    },
    // ===== ReplicatedRecord =====
    //
    /// Response to [`GetReplicatedRecord`]
    ///
    /// [`GetReplicatedRecord`]: crate::messages::Query::GetReplicatedRecord
    GetReplicatedRecord(Result<(NetworkAddress, Bytes)>),
    // ===== ReplicatedRecord =====
    //
    /// Response to [`GetChunkExistenceProof`]
    ///
    /// [`GetChunkExistenceProof`]: crate::messages::Query::GetChunkExistenceProof
    GetChunkExistenceProof(Result<ChunkProof>),
}

// Debug implementation for QueryResponse, to avoid printing Vec<u8>
impl Debug for QueryResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QueryResponse::GetStoreCost {
                quote,
                payment_address,
                peer_address,
            } => {
                write!(
                    f,
                    "GetStoreCost(quote: {quote:?}, from {peer_address:?} w/ payment_address: {payment_address:?})"
                )
            }
            QueryResponse::CheckNodeInProblem {
                reporter_address,
                target_address,
                is_in_trouble,
            } => {
                write!(
                    f,
                    "CheckNodeInProblem({reporter_address:?} report target {target_address:?} as {is_in_trouble:?} in problem"
                )
            }
            QueryResponse::GetReplicatedRecord(result) => match result {
                Ok((holder, data)) => {
                    write!(
                        f,
                        "GetReplicatedRecord(Ok((holder: {:?}, datalen: {:?})))",
                        holder,
                        data.len()
                    )
                }
                Err(err) => {
                    write!(f, "GetReplicatedRecord(Err({err:?}))")
                }
            },
            QueryResponse::GetChunkExistenceProof(proof) => {
                write!(f, "GetChunkExistenceProof(proof: {proof:?})")
            }
        }
    }
}

/// The response to a Cmd, containing the query result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CmdResponse {
    //
    // ===== Replication =====
    //
    /// Response to replication cmd
    Replicate(Result<()>),
    //
    // ===== QuoteVerification =====
    //
    /// Response to quote verification cmd
    QuoteVerification(Result<()>),
    //
    // ===== PeerConsideredAsBad =====
    //
    /// Response to the considered as bad notification
    PeerConsideredAsBad(Result<()>),
}

/// The Ok variant of a CmdResponse
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CmdOk {
    StoredSuccessfully,
    DataAlreadyPresent,
}
