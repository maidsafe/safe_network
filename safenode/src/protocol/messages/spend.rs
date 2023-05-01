// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::protocol::{
    error::TransferError,
    storage::{dbc_address, DbcAddress},
};

use sn_dbc::{DbcId, DerivationIndex, MainKey, RevealedAmount};

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
    /// Returns the dst address for the query.
    pub fn dst(&self) -> DbcAddress {
        match self {
            Self::GetFees { dbc_id, .. } => dbc_address(dbc_id),
            Self::GetDbcSpend(ref address) => *address,
        }
    }
}

impl std::fmt::Display for SpendQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GetFees { dbc_id, priority } => {
                write!(f, "SpendQuery::GetFees({dbc_id:?}, {priority:?})")
            }
            Self::GetDbcSpend(address) => {
                write!(f, "SpendQuery::GetDbcSpend({:?})", address)
            }
        }
    }
}

// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

/// Used by client to choose how fast their spend will be processed.
/// The chosen variant will map to a fee using the spend queue stats fetched from Nodes.
#[derive(
    Copy, Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize,
)]
pub enum SpendPriority {
    /// `High` + 1 std dev.
    Highest,
    /// The highest fee in spend queue.
    High,
    /// Avg of `High` and `Normal`.
    MediumHigh,
    /// The avg fee in spend queue.
    Normal,
    /// Avg of `Normal` and `Low`.
    MediumLow,
    /// The lowest fee in spend queue.
    Low,
    /// `Low` - 1 std dev.
    Lowest,
}

/// These are sent with a spend, so that a Node
/// can verify that the transfer fee is being paid.
///
/// A client asks for the fee for a spend, and a Node returns
/// a cipher of the amount and a blinding factor, i.e. a `RevealedAmount`.
/// The Client decrypts it and uses the amount and blinding factor to build
/// the payment dbc to the Node. The amount + blinding factor is then
/// encrypted to a _derived_ key of the Node reward key.
/// The client also encrypts the derivation index used, to the Node _reward key_,
/// and sends both the amount + blinding factor cipher and the derivation index cipher
/// to the Node by including this `FeeCiphers` struct in the spend cmd.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct FeeCiphers {
    amount: bls::Ciphertext,
    derivation_index: bls::Ciphertext,
}

impl FeeCiphers {
    /// Creates a new FeeCiphers struct.
    pub fn new(amount: bls::Ciphertext, derivation_index: bls::Ciphertext) -> Self {
        Self {
            amount,
            derivation_index,
        }
    }

    /// Decrypts the derivation index cipher using the reward `MainKey`, then gets the `DerivedKey`
    /// that was used to decrypt the amount cipher, giving the `RevealedAmount` containing amount and blinding factor.
    /// Returns the `RevealedAmount`, and the DbcId corresponding to the `DerivedKey`.
    pub fn decrypt(
        &self,
        node_reward_key: &MainKey,
    ) -> Result<(DbcId, RevealedAmount), TransferError> {
        let derivation_index = self.decrypt_derivation_index(node_reward_key)?;
        let derived_key = node_reward_key.derive_key(&derivation_index);

        let dbc_id = derived_key.dbc_id();
        let amount = RevealedAmount::try_from((&derived_key, &self.amount))?;

        Ok((dbc_id, amount))
    }

    /// The derivation index is encrypted to the Node `PublicAddress` for rewards.
    /// The `DerivedKey` which can be derived from the Node reward `MainKey` using that index, is then used to decrypt the amount cihper.
    fn decrypt_derivation_index(
        &self,
        node_reward_key: &MainKey,
    ) -> Result<DerivationIndex, TransferError> {
        let bytes = node_reward_key.decrypt_index(&self.derivation_index)?;

        let mut index = [0u8; 32];
        index.copy_from_slice(&bytes[0..32]);

        Ok(index)
    }
}
