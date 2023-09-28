// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{error::Result, NetworkAddress};

use serde::{Deserialize, Serialize};
use sn_transfers::{MainPubkey, NanoTokens};
use std::fmt::Debug;

/// The response to a query, containing the query result.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, custom_debug::Debug)]
pub enum QueryResponse {
    GetStoreCost {
        /// The store cost in nanos for storing the next record.
        store_cost: Result<NanoTokens>,
        /// The cash_note MainPubkey to pay this node's store cost to.
        payment_address: MainPubkey,
    },
    // ===== ReplicatedRecord =====
    //
    /// Response to [`GetReplicatedRecord`]
    ///
    /// [`GetReplicatedRecord`]: crate::messages::Query::GetReplicatedRecord
    GetReplicatedRecord(Result<(NetworkAddress, Vec<u8>)>),
}

/// The response to a Cmd, containing the query result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CmdResponse {
    //
    // ===== Replication =====
    //
    /// Response to replication cmd
    Replicate(Result<()>),
}

/// The Ok variant of a CmdResponse
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CmdOk {
    StoredSuccessfully,
    DataAlreadyPresent,
}
