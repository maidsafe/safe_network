// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{Error, Result};

use super::UniquePubkey;

use serde::{Deserialize, Serialize};
use std::{fmt, hash::Hash};
use xor_name::XorName;

/// The address of a SignedSpend in the network.
/// This is used to check if a CashNote is spent, note that the actual CashNote is not stored on the Network.
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct SpendAddress(XorName);

impl SpendAddress {
    /// Construct a `SpendAddress` given an `XorName`.
    pub fn new(name: XorName) -> Self {
        Self(name)
    }

    /// Construct a `SpendAddress` from a `UniquePubkey`.
    pub fn from_unique_pubkey(unique_pubkey: &UniquePubkey) -> Self {
        Self::new(XorName::from_content(&unique_pubkey.to_bytes()))
    }

    /// Return the name, which is the hash of `UniquePubkey`.
    pub fn xorname(&self) -> &XorName {
        &self.0
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    pub fn from_hex(hex: &str) -> Result<Self> {
        let bytes = hex::decode(hex).map_err(|e| Error::HexDeserializationFailed(e.to_string()))?;
        let xorname = XorName(
            bytes
                .try_into()
                .map_err(|_| Error::HexDeserializationFailed("wrong string size".to_string()))?,
        );
        Ok(Self::new(xorname))
    }
}

impl std::fmt::Debug for SpendAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SpendAddress({})", &self.to_hex()[0..6])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spend_address_hex_conversions() -> eyre::Result<()> {
        let mut rng = rand::thread_rng();
        let spend_address = SpendAddress::new(XorName::random(&mut rng));
        let hex = spend_address.to_hex();
        let spend_address2 = SpendAddress::from_hex(&hex)?;
        assert_eq!(spend_address, spend_address2);
        Ok(())
    }
}
