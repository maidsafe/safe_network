// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use serde::{Deserialize, Serialize};
use sn_dbc::DbcId;
use std::hash::Hash;
use xor_name::XorName;

/// The address of a Dbc in the network.
/// This is used to find information of if it is spent, not to store the actual Dbc.
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Debug)]
pub struct DbcAddress(XorName);

impl DbcAddress {
    /// Construct a `DbcAddress` given the name of a `DbcId`.
    pub fn new(name: XorName) -> Self {
        Self(name)
    }

    /// Returns the name, which is the `DbcId` XorName.
    pub fn name(&self) -> &XorName {
        &self.0
    }
}

/// Still thinking of best location for this.
/// Wanted to make the DbcAddress take a dbc id actually..
pub fn dbc_address(dbc_id: &DbcId) -> DbcAddress {
    DbcAddress::new(dbc_name(dbc_id))
}

/// Still thinking of best location for this.
/// Wanted to make the DbcAddress take a dbc id actually..
pub fn dbc_name(dbc_id: &DbcId) -> XorName {
    XorName::from_content(&dbc_id.to_bytes())
}
