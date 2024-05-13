// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

use crate::TransferError;

/// sha3 256 hash used for Spend Reasons, Transaction hashes, anything hash related in this crate
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize)]
pub struct Hash([u8; 32]);

impl Hash {
    #[allow(clippy::self_named_constructors)]
    /// sha3 256 hash
    pub fn hash(input: &[u8]) -> Self {
        Self::from(sha3_256(input))
    }

    /// Access the 32 byte slice of the hash
    pub fn slice(&self) -> &[u8; 32] {
        &self.0
    }

    /// Deserializes a `Hash` represented as a hex string to a `Hash`.
    pub fn from_hex(hex: &str) -> Result<Self, TransferError> {
        let mut h = Self::default();
        hex::decode_to_slice(hex, &mut h.0)
            .map_err(|e| TransferError::HexDeserializationFailed(e.to_string()))?;
        Ok(h)
    }

    /// Serialize this `Hash` instance to a hex string.
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }
}

impl FromStr for Hash {
    type Err = TransferError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Hash::from_hex(s)
    }
}

impl From<[u8; 32]> for Hash {
    fn from(val: [u8; 32]) -> Hash {
        Hash(val)
    }
}

// Display Hash value as hex in Debug output.  consolidates 36 lines to 3 for pretty output
impl fmt::Debug for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Hash").field(&self.to_hex()).finish()
    }
}

impl AsRef<[u8]> for Hash {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

pub(crate) fn sha3_256(input: &[u8]) -> [u8; 32] {
    use tiny_keccak::{Hasher, Sha3};

    let mut sha3 = Sha3::v256();
    let mut output = [0; 32];
    sha3.update(input);
    sha3.finalize(&mut output);
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash() {
        let data = b"hello world";
        let expected = b"\
            \x64\x4b\xcc\x7e\x56\x43\x73\x04\x09\x99\xaa\xc8\x9e\x76\x22\xf3\
            \xca\x71\xfb\xa1\xd9\x72\xfd\x94\xa3\x1c\x3b\xfb\xf2\x4e\x39\x38\
        ";
        assert_eq!(sha3_256(data), *expected);

        let hash = Hash::hash(data);
        assert_eq!(hash.slice(), expected);
    }

    #[test]
    fn hex_encoding() {
        let data = b"hello world";
        let expected_hex = "644bcc7e564373040999aac89e7622f3ca71fba1d972fd94a31c3bfbf24e3938";

        let hash = Hash::hash(data);

        assert_eq!(hash.to_hex(), expected_hex.to_string());
        assert_eq!(Hash::from_hex(expected_hex), Ok(hash));

        let too_long_hex = format!("{expected_hex}ab");
        assert_eq!(
            Hash::from_hex(&too_long_hex),
            Err(TransferError::HexDeserializationFailed(
                "Invalid string length".to_string()
            ))
        );

        assert_eq!(
            Hash::from_hex(&expected_hex[0..30]),
            Err(TransferError::HexDeserializationFailed(
                "Invalid string length".to_string()
            ))
        );
    }
}
