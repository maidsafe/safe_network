// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::error::{Error, Result};

use bls::{PublicKey, PK_SIZE};
use serde::{Deserialize, Serialize};
use std::{
    fmt::{Debug, Display},
    hash::Hash,
};
use xor_name::{XorName, XOR_NAME_LEN};

/// Address of a Register on the SAFE Network
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct RegisterAddress {
    /// User chosen meta, can be anything, the register's name on the network will be the hash of this meta and the owner
    pub(crate) meta: XorName,
    /// Owner of the register
    pub(crate) owner: PublicKey,
}

impl Display for RegisterAddress {
    /// Display the register address in hex format that can be parsed by `RegisterAddress::from_hex`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", &self.to_hex())
    }
}

impl Debug for RegisterAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "RegisterAddress({}) {{ meta: {:?}, owner: {:?} }}",
            &self.to_hex()[0..6],
            self.meta,
            self.owner
        )
    }
}

impl RegisterAddress {
    /// Construct a new `RegisterAddress` given `meta` and `owner`.
    pub fn new(meta: XorName, owner: PublicKey) -> Self {
        Self { meta, owner }
    }

    /// Return the network name of the register.
    /// This is used to locate the register on the network.
    pub fn xorname(&self) -> XorName {
        let mut bytes = vec![];
        bytes.extend_from_slice(&self.meta.0);
        bytes.extend_from_slice(&self.owner.to_bytes());
        XorName::from_content(&bytes)
    }

    /// Serialize this `RegisterAddress` instance to a hex-encoded `String`.
    pub fn to_hex(&self) -> String {
        let mut bytes = vec![];
        bytes.extend_from_slice(&self.meta.0);
        bytes.extend_from_slice(&self.owner.to_bytes());
        hex::encode(bytes)
    }

    /// Deserialize a hex-encoded representation of a `RegisterAddress` to a `RegisterAddress` instance.
    pub fn from_hex(hex: &str) -> Result<Self> {
        let bytes = hex::decode(hex).map_err(|_| Error::HexDeserializeFailed)?;
        let meta_bytes: [u8; XOR_NAME_LEN] = bytes[..XOR_NAME_LEN]
            .try_into()
            .map_err(|_| Error::HexDeserializeFailed)?;
        let meta = XorName(meta_bytes);
        let owner_bytes: [u8; PK_SIZE] = bytes[XOR_NAME_LEN..]
            .try_into()
            .map_err(|_| Error::HexDeserializeFailed)?;
        let owner = PublicKey::from_bytes(owner_bytes).map_err(|_| Error::HexDeserializeFailed)?;
        Ok(Self { meta, owner })
    }

    /// Return the user chosen meta.
    pub fn meta(&self) -> XorName {
        self.meta
    }

    /// Return the owner.
    pub fn owner(&self) -> PublicKey {
        self.owner
    }
}

#[cfg(test)]
mod tests {
    use bls::SecretKey;

    use super::*;

    #[test]
    fn test_register_hex_conversion() {
        let mut rng = rand::thread_rng();
        let owner = SecretKey::random().public_key();
        let meta = XorName::random(&mut rng);
        let addr = RegisterAddress::new(meta, owner);
        let hex = &addr.to_hex();
        let addr2 = RegisterAddress::from_hex(hex).unwrap();

        assert_eq!(addr, addr2);

        let bad_hex = format!("{hex}0");
        let err = RegisterAddress::from_hex(&bad_hex);
        assert_eq!(err, Err(Error::HexDeserializeFailed));
    }
}
