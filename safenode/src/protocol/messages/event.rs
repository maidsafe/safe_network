// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    protocol::types::address::{dbc_address, DataAddress},
    transfers::{Error, Result},
};

use sn_dbc::SignedSpend;

use serde::{Deserialize, Serialize};

/// Events - creating, updating, or removing data.
///
/// See the [`types`] module documentation for more details of the types supported by the Safe
/// Network, and their semantics.
///
/// [`types`]: crate::protocol::types
#[allow(clippy::large_enum_variant)]
#[derive(Eq, PartialEq, Clone, Serialize, Deserialize, Debug)]
pub enum Event {
    /// A peer detected a double spend attempt for a [`SignedSpend`].
    /// Contains the first two spends of same id that were detected as being different.
    ///
    /// [`SignedSpend`]: sn_dbc::SignedSpend
    DoubleSpendAttempted(Box<SignedSpend>, Box<SignedSpend>),
}

impl Event {
    /// Used to send a cmd to the close group of the address.
    pub fn dst(&self) -> DataAddress {
        match self {
            Event::DoubleSpendAttempted(a, _) => DataAddress::Spend(dbc_address(a.dbc_id())),
        }
    }

    /// Create a new [`Event::DoubleSpendAttempted`] event.
    /// It is validated so that only two spends with same id
    /// can be used to create this event.
    pub fn double_spend_attempt(a: Box<SignedSpend>, b: Box<SignedSpend>) -> Result<Self> {
        if a.dbc_id() == b.dbc_id() {
            Ok(Event::DoubleSpendAttempted(a, b))
        } else {
            Err(Error::NotADoubleSpendAttempt(a, b))
        }
    }
}
