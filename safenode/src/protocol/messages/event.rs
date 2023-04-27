// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::domain::{
    node_transfers::{Error, Result},
    storage::{dbc_address, DataAddress, Error as StorageError},
};

use sn_dbc::SignedSpend;

use serde::{Deserialize, Serialize};

/// Events - creating, updating, or removing data.
///
/// See the [`protocol`] module documentation for more details of the types supported by the Safe
/// Network, and their semantics.
///
/// [`protocol`]: crate::protocol
#[allow(clippy::large_enum_variant)]
#[derive(Eq, PartialEq, Clone, Serialize, Deserialize, custom_debug::Debug)]
pub enum Event {
    /// A peer detected a double spend attempt for a [`SignedSpend`].
    /// Contains the first two spends of same id that were detected as being different.
    ///
    /// [`SignedSpend`]: sn_dbc::SignedSpend
    #[debug(skip)]
    DoubleSpendAttempted {
        /// New spend that we received.
        #[debug(skip)]
        new: Box<SignedSpend>,
        /// Existing spend of same id that we already have.
        #[debug(skip)]
        existing: Box<SignedSpend>,
    },
}

impl Event {
    /// Used to send a cmd to the close group of the address.
    pub fn dst(&self) -> DataAddress {
        match self {
            Event::DoubleSpendAttempted { new, .. } => {
                DataAddress::Spend(dbc_address(new.dbc_id()))
            }
        }
    }

    /// Create a new [`Event::DoubleSpendAttempted`] event.
    /// It is validated so that only two spends with same id
    /// can be used to create this event.
    pub fn double_spend_attempt(new: Box<SignedSpend>, existing: Box<SignedSpend>) -> Result<Self> {
        if new.dbc_id() == existing.dbc_id() {
            Ok(Event::DoubleSpendAttempted { new, existing })
        } else {
            // If the ids are different, then this is not a double spend attempt.
            // A double spend attempt is when the contents (the tx) of two spends
            // with same id are detected as being different.
            // A node could erroneously send a notification of a double spend attempt,
            // so, we need to validate that.
            Err(Error::Storage(StorageError::NotADoubleSpendAttempt {
                one: new,
                other: existing,
            }))
        }
    }
}
