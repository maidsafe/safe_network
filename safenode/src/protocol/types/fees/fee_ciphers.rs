// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::Result;

use sn_dbc::{DbcId, DerivationIndex, MainKey, RevealedAmount};

use serde::{Deserialize, Serialize};
use std::fmt::Debug;

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
    #[allow(clippy::result_large_err)]
    pub fn decrypt(&self, node_reward_key: &MainKey) -> Result<(DbcId, RevealedAmount)> {
        let derivation_index = self.decrypt_derivation_index(node_reward_key)?;
        let derived_key = node_reward_key.derive_key(&derivation_index);

        let dbc_id = derived_key.dbc_id();
        let amount = RevealedAmount::try_from((&derived_key, &self.amount))?;

        Ok((dbc_id, amount))
    }

    /// The derivation index is encrypted to the Node `PublicAddress` for rewards.
    /// The `DerivedKey` which can be derived from the Node reward `MainKey` using that index, is then used to decrypt the amount cihper.
    #[allow(clippy::result_large_err)]
    fn decrypt_derivation_index(&self, node_reward_key: &MainKey) -> Result<DerivationIndex> {
        let bytes = node_reward_key.decrypt_index(&self.derivation_index)?;

        let mut index = [0u8; 32];
        index.copy_from_slice(&bytes[0..32]);

        Ok(index)
    }
}
