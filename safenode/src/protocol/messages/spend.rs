// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::protocol::types::{
    address::{dbc_address, DbcAddress},
    fees::SpendPriority,
};

use sn_dbc::DbcId;

use serde::{Deserialize, Serialize};

/// A spend related query to the network.
#[derive(Eq, PartialEq, PartialOrd, Clone, Serialize, Deserialize, Debug)]
pub enum SpendQuery {
    /// Query for the current fee for processing a `Spend` of a Dbc with the given id.
    GetFees {
        /// The id of the Dbc to spend.
        dbc_id: DbcId,
        /// The priority of the spend.
        priority: SpendPriority,
    },
    /// Query for a `Spend` of a Dbc with at the given address.
    GetDbcSpend(DbcAddress),
}

impl SpendQuery {
    /// Returns the dst address for the request.
    pub fn dst(&self) -> DbcAddress {
        match self {
            Self::GetFees { dbc_id, .. } => dbc_address(dbc_id),
            Self::GetDbcSpend(ref address) => *address,
        }
    }
}
