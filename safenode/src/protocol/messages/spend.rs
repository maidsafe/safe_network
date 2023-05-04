// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::protocol::storage::DbcAddress;

use serde::{Deserialize, Serialize};

/// A spend related query to the network.
#[derive(Eq, PartialEq, PartialOrd, Clone, Serialize, Deserialize, Debug)]
pub enum SpendQuery {
    /// Query for a `Spend` of a Dbc with at the given address.
    GetDbcSpend(DbcAddress),
}

impl SpendQuery {
    /// Returns the dst address for the query.
    pub fn dst(&self) -> DbcAddress {
        match self {
            Self::GetDbcSpend(ref address) => *address,
        }
    }
}

impl std::fmt::Display for SpendQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GetDbcSpend(address) => {
                write!(f, "SpendQuery::GetDbcSpend({:?})", address)
            }
        }
    }
}
