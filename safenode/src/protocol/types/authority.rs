// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

pub use bls::{PublicKey, Signature};

use super::errors::{Error, Result};

use serde::{Deserialize, Serialize};

/// Authority over a piece of content and/or associated operations.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct DataAuthority {
    /// Public key.
    pub public_key: PublicKey,
    /// Signature.
    pub signature: Signature,
}

impl DataAuthority {
    /// Verify the authority over the provided `payload`.
    pub fn verify_authority(&self, payload: impl AsRef<[u8]>) -> Result<()> {
        if self.public_key.verify(&self.signature, payload) {
            Ok(())
        } else {
            Err(Error::InvalidSignature(self.public_key))
        }
    }
}
