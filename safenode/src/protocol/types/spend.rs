// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::address::{dbc_name, DbcAddress};

use sn_dbc::{DbcId, SignedSpend};

use serde::{Deserialize, Serialize};
use xor_name::XorName;

/// An immutable Spend of a Dbc, meaning that the
/// value of it has been transferred to other Dbc(s).
#[derive(Hash, Eq, PartialEq, Clone, custom_debug::Debug, Serialize, Deserialize)]
pub struct Spend {
    /// Network address of a Dbc, where its spend will be recorded.
    address: DbcAddress,
    /// Contained `SignedSpend`.
    #[debug(skip)]
    signed_spend: SignedSpend,
}

impl Spend {
    /// Create a new instance of `Spend`.
    pub fn new(dbc_id: DbcId, signed_spend: SignedSpend) -> Self {
        Self {
            address: DbcAddress::new(dbc_name(&dbc_id)),
            signed_spend,
        }
    }

    /// Return the id.
    pub fn id(&self) -> &XorName {
        self.address.name()
    }

    /// Return the address.
    pub fn address(&self) -> &DbcAddress {
        &self.address
    }

    /// Return the value.
    pub fn signed_spend(&self) -> &SignedSpend {
        &self.signed_spend
    }
}
