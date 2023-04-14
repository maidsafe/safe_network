// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::Result;

use sn_dbc::{Ciphertext, DbcId, DerivedKey, Hash, PublicAddress, RevealedAmount, Token};

use serde::{Deserialize, Serialize};
use tiny_keccak::{Hasher, Sha3};

/// The content of a required fee.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct RequiredFeeContent {
    /// A simple u64, encrypted to the `DbcId` of the `Dbc` being spent,
    /// for which a fee is being required.
    pub amount_cipher: Ciphertext,
    /// Node's `PublicAddress` for rewards. Used to derive
    /// a `DbcId` for a new `Dbc` with the fee payment in it.
    /// Deriving a `DbcId` from a `PublicAddress`, means that the holder
    /// of the `MainKey` corresponding to this `PublicAddress`, can access
    /// the tokens in that `Dbc`.
    pub reward_address: PublicAddress,
}

impl RequiredFeeContent {
    /// Create `RequiredFeeContent` from the fee amount, the id of the `Dbc` to spend, and Node reward `PublicAddress`.
    /// The `DbcId` is used to encrypt the amount, so that only the holder of the `Dbc` to spend can see the fee amount.
    pub fn new(amount: Token, dbc_id: DbcId, reward_address: PublicAddress) -> Self {
        let revealed_amount = RevealedAmount::from_amount(amount.as_nano(), rand::thread_rng());
        Self {
            amount_cipher: dbc_id.encrypt(&revealed_amount),
            reward_address,
        }
    }

    /// Represent as byte array.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut v: Vec<u8> = Default::default();
        v.extend(&self.amount_cipher.to_bytes());
        v.extend(&self.reward_address.to_bytes());
        v
    }

    /// Generate hash.
    pub fn hash(&self) -> Hash {
        let mut sha3 = Sha3::v256();
        sha3.update(&self.to_bytes());
        let mut hash = [0; 32];
        sha3.finalize(&mut hash);
        Hash::from(hash)
    }

    /// Decrypts the amount using the `DerivedKey` of the `Dbc` to spend.
    #[allow(clippy::result_large_err)]
    pub fn decrypt_amount(&self, derived_key: &DerivedKey) -> Result<Token> {
        let amount = RevealedAmount::try_from((derived_key, &self.amount_cipher))?;
        Ok(Token::from_nano(amount.value()))
    }
}
