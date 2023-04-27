// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use serde::{Deserialize, Serialize};
use std::hash::Hash;
use xor_name::XorName;

/// Address of a Register, different from
/// a `ChunkAddress` in that it also includes a tag.
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Debug)]
pub struct RegisterAddress {
    /// Name.
    pub name: XorName,
    /// Tag.
    pub tag: u64,
}

impl RegisterAddress {
    /// Construct a new `RegisterAddress` given `name` and `tag`.
    pub fn new(name: XorName, tag: u64) -> Self {
        Self { name, tag }
    }

    /// Return the identifier of the register.
    /// This is used to locate the register on the network.
    pub fn id(&self) -> XorName {
        let mut bytes = vec![];
        bytes.extend_from_slice(&self.name.0);
        bytes.extend_from_slice(&self.tag.to_be_bytes());
        XorName::from_content(&bytes)
    }

    /// Return the name.
    /// This is not a unique identifier.
    pub fn name(&self) -> &XorName {
        &self.name
    }

    /// Return the tag.
    pub fn tag(&self) -> u64 {
        self.tag
    }
}
