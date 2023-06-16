// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use serde::{Deserialize, Serialize};
use sn_dbc::{DbcTransaction, SignedSpend};
use std::hash::{Hash, Hasher};

type ParentTx = DbcTransaction;

/// The `SignedSpend` along with its corresponding `ParentTx`
#[derive(custom_debug::Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct SpendWithParent {
    /// The dbc to register in the network as spent.
    pub signed_spend: SignedSpend,
    /// The dbc transaction that the spent dbc was created in.
    #[debug(skip)]
    pub parent_tx: ParentTx,
}

// ParentTx does not impl Hash. So just hash SignedSpend
impl Hash for SpendWithParent {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.signed_spend.hash(state);
    }
}
