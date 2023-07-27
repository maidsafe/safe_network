// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{register::MAX_REG_ENTRY_SIZE, Error};

use serde::{Deserialize, Serialize};
use std::{
    fmt::{Debug, Display, Formatter, Result as FmtResult},
    result::Result,
};
use xor_name::XorName;

/// Metadata of a Register, provided by the creator (end user) upon creation, which becomes immutable,
/// and it defines this Register's address on the network, i.e. this Register is stored by the network
/// at: XorName(hash(medatada)) (note that the size is limited: `MAX_REG_ENTRY_SIZE`).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd)]
pub struct Metadata(Vec<u8>);

impl Metadata {
    /// Creates a new Metadata checking the data length is not larger than the allowed max size.
    pub fn new(metadata: &[u8]) -> Result<Self, Error> {
        let data = metadata.to_vec();
        if data.len() > MAX_REG_ENTRY_SIZE {
            return Err(Error::MetadataTooBig {
                size: data.len(),
                max: MAX_REG_ENTRY_SIZE,
            });
        }

        Ok(Self(data))
    }

    /// Returns the xorname this metadata would be mapped to.
    pub fn xorname(&self) -> XorName {
        XorName::from_content(&self.0)
    }
}

/// An entry in a Register (note that the `vec<u8>` is size limited: `MAX_REG_ENTRY_SIZE`)
pub type Entry = Vec<u8>;

/// Hash of the register entry. Logging as the same format of `XorName`.
#[derive(Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntryHash(pub crdts::merkle_reg::Hash);

impl Debug for EntryHash {
    fn fmt(&self, formatter: &mut Formatter) -> FmtResult {
        write!(formatter, "{self}")
    }
}

impl Display for EntryHash {
    fn fmt(&self, formatter: &mut Formatter) -> FmtResult {
        write!(
            formatter,
            "{:02x}{:02x}{:02x}..",
            self.0[0], self.0[1], self.0[2]
        )
    }
}
