// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.
#![allow(dead_code)]
#![allow(clippy::result_large_err)]

#[macro_use]
extern crate tracing;

/// Genesis utilities.
pub mod genesis;
/// Client handling of token transfers.
pub mod transfers;
/// A wallet for network tokens.
pub mod wallet;

mod address;
mod builder;
mod cashnote;
mod error;
mod fee_output;
mod nano;
mod signed_spend;
mod transaction;
mod unique_keys;

// re-export crates used in our public API
pub use crate::{
    address::SpendAddress,
    builder::TransactionBuilder,
    cashnote::CashNote,
    error::{Error, Result},
    fee_output::FeeOutput,
    nano::NanoTokens,
    signed_spend::{SignedSpend, Spend},
    transaction::{Input, Output, Transaction},
    unique_keys::{DerivationIndex, DerivedSecretKey, MainPubkey, MainSecretKey, UniquePubkey},
};
pub use bls::{self, rand, Ciphertext, PublicKey, PublicKeySet, Signature, SignatureShare};

use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

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
    pub fn from_hex(hex: &str) -> Result<Self, Error> {
        let mut h = Self::default();
        hex::decode_to_slice(hex, &mut h.0)
            .map_err(|e| Error::HexDeserializationFailed(e.to_string()))?;
        Ok(h)
    }

    /// Serialize this `Hash` instance to a hex string.
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }
}

impl FromStr for Hash {
    type Err = Error;

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

/// This is a helper module to make it a bit easier
/// and regular for API callers to instantiate
/// an Rng when calling sn_transfers methods that require
/// them.
pub mod rng {
    use crate::rand::{
        rngs::{StdRng, ThreadRng},
        SeedableRng,
    };

    pub fn thread_rng() -> ThreadRng {
        crate::rand::thread_rng()
    }

    pub fn from_seed(seed: <StdRng as SeedableRng>::Seed) -> StdRng {
        StdRng::from_seed(seed)
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
    use quickcheck::{Arbitrary, Gen};

    #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    pub struct TinyInt(pub u8);

    impl TinyInt {
        pub fn coerce<T: From<u8>>(self) -> T {
            self.0.into()
        }
    }

    impl std::fmt::Debug for TinyInt {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    impl Arbitrary for TinyInt {
        fn arbitrary(g: &mut Gen) -> Self {
            Self(u8::arbitrary(g) % 5)
        }

        fn shrink(&self) -> Box<dyn Iterator<Item = Self>> {
            Box::new((0..(self.0)).rev().map(Self))
        }
    }

    #[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
    pub struct TinyVec<T>(pub Vec<T>);

    impl<T> TinyVec<T> {
        pub fn into_iter(self) -> impl Iterator<Item = T> {
            self.0.into_iter()
        }
    }

    impl<T: std::fmt::Debug> std::fmt::Debug for TinyVec<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{:?}", self.0)
        }
    }

    impl<T: Arbitrary> Arbitrary for TinyVec<T> {
        fn arbitrary(g: &mut Gen) -> Self {
            let n = u8::arbitrary(g) % 7;
            let mut vec = Vec::new();
            for _ in 0..n {
                vec.push(T::arbitrary(g));
            }
            Self(vec)
        }

        fn shrink(&self) -> Box<dyn Iterator<Item = Self>> {
            Box::new(self.0.shrink().map(Self))
        }
    }

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
            Err(Error::HexDeserializationFailed(
                "Invalid string length".to_string()
            ))
        );

        assert_eq!(
            Hash::from_hex(&expected_hex[0..30]),
            Err(Error::HexDeserializationFailed(
                "Invalid string length".to_string()
            ))
        );
    }
}
