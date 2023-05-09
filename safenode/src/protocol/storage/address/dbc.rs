// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use sn_dbc::DbcId;

use serde::{Deserialize, Serialize};
use std::hash::Hash;
use xor_name::XorName;

/// The address of a Dbc in the network.
/// This is used to find information of if it is spent, not to store the actual Dbc.
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Debug)]
pub struct DbcAddress(XorName);

impl DbcAddress {
    /// Construct a `DbcAddress` given an `XorName`.
    pub fn new(name: XorName) -> Self {
        Self(name)
    }

    /// Construct a `DbcAddress` from a `DbcId`.
    pub fn from_dbc_id(dbc_id: &DbcId) -> Self {
        Self::new(XorName::from_content(&dbc_id.to_bytes()))
    }

    /// Return the name, which is the hash of `DbcId`.
    pub fn name(&self) -> &XorName {
        &self.0
    }
}
