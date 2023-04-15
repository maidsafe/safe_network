// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{Error, RequiredFeeContent, Result};

use sn_dbc::{DbcId, Hash, MainKey, Signature, Token};

use serde::{Deserialize, Serialize};
use tiny_keccak::{Hasher, Sha3};

/// A Node responds to a Client who wishes to spend a dbc,
/// informing the Client of the required fee for the spend.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct RequiredFee {
    /// The content of the RequiredFee.
    pub content: RequiredFeeContent,
    /// The signature over the content, by the reward address.
    pub reward_address_sig: Signature,
}

impl RequiredFee {
    /// Instantiate RequiredFee by encrypting the amount to the id of the dbc to spend, and signing
    /// it all with the Node reward main key.
    pub fn new(amount: Token, dbc_id: DbcId, reward_main_key: &MainKey) -> Self {
        let content = RequiredFeeContent::new(amount, dbc_id, reward_main_key.public_address());
        let reward_address_sig = reward_main_key.sign(&content.to_bytes());
        Self {
            content,
            reward_address_sig,
        }
    }

    /// Verifies that reward_address_sig is correct.
    #[allow(clippy::result_large_err)]
    pub fn verify(&self) -> Result<()> {
        let valid = self
            .content
            .reward_address
            .verify(&self.reward_address_sig, &self.content.to_bytes());

        match valid {
            true => Ok(()),
            false => Err(Error::RequiredFeeSignatureInvalid),
        }
    }

    /// Represent RequiredFee as bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut v: Vec<u8> = Default::default();
        v.extend(&self.content.to_bytes());
        v.extend(&self.reward_address_sig.to_bytes());
        v
    }

    /// Generate hash of RequiredFee.
    pub fn hash(&self) -> Hash {
        let mut sha3 = Sha3::v256();
        sha3.update(&self.to_bytes());
        let mut hash = [0; 32];
        sha3.finalize(&mut hash);
        Hash::from(hash)
    }
}
