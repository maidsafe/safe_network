// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use serde::{Deserialize, Serialize};

use crate::PaymentQuote;

// NB TODO actually implement the placeholders below: RewardsAddress and ProofOfPayment

/// The address rewards should be sent to
#[derive(Debug, Clone, PartialEq, Eq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct RewardsAddress(String);

impl RewardsAddress {
    pub fn new(address: String) -> Self {
        Self(address)
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl RewardsAddress {
    pub fn dummy() -> Self {
        Self("dummy".to_string())
    }
}

/// The proof of payment for a data payment
#[derive(Debug, Clone, PartialEq, Eq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct ProofOfPayment {
    /// The Quote we're paying for
    pub quote: PaymentQuote,
}
