// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::error::{Error, Result};
use bls::PublicKey;
use serde::{Deserialize, Serialize};
use std::{
    fmt::{Debug, Display},
    hash::Hash,
};
use xor_name::XorName;

/// Address of a Scratchpad on the SAFE Network
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct ScratchpadAddress {
    /// Owner of the scratchpad
    pub(crate) owner: PublicKey,
}

impl Display for ScratchpadAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({:?})", &self.to_hex()[0..6])
    }
}

impl Debug for ScratchpadAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ScratchpadAddress({}) {{ owner: {:?} }}",
            &self.to_hex()[0..6],
            self.owner
        )
    }
}

impl ScratchpadAddress {
    /// Construct a new `ScratchpadAddress` given `owner`.
    pub fn new(owner: PublicKey) -> Self {
        Self { owner }
    }

    /// Return the network name of the scratchpad.
    /// This is used to locate the scratchpad on the network.
    pub fn xorname(&self) -> XorName {
        XorName::from_content(&self.owner.to_bytes())
    }

    /// Serialize this `ScratchpadAddress` instance to a hex-encoded `String`.
    pub fn to_hex(&self) -> String {
        hex::encode(self.owner.to_bytes())
    }

    /// Deserialize a hex-encoded representation of a `ScratchpadAddress` to a `ScratchpadAddress` instance.
    pub fn from_hex(hex: &str) -> Result<Self> {
        // let bytes = hex::decode(hex).map_err(|_| Error::ScratchpadHexDeserializeFailed)?;
        let owner = PublicKey::from_hex(hex).map_err(|_| Error::ScratchpadHexDeserializeFailed)?;
        Ok(Self { owner })
    }

    /// Return the owner.
    pub fn owner(&self) -> &PublicKey {
        &self.owner
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bls::SecretKey;

    #[test]
    fn test_scratchpad_hex_conversion() {
        let owner = SecretKey::random().public_key();
        let addr = ScratchpadAddress::new(owner);
        let hex = addr.to_hex();
        let addr2 = ScratchpadAddress::from_hex(&hex).unwrap();

        assert_eq!(addr, addr2);

        let bad_hex = format!("{hex}0");
        let err = ScratchpadAddress::from_hex(&bad_hex);
        assert_eq!(err, Err(Error::ScratchpadHexDeserializeFailed));
    }
}
