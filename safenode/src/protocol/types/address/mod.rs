// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod chunk;
mod register;
mod spentbook;

pub use chunk::ChunkAddress;
pub use register::RegisterAddress;
pub use spentbook::SpentbookAddress;

use serde::{Deserialize, Serialize};
use xor_name::XorName;

/// An address of data on the network
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Debug)]
pub enum DataAddress {
    ///
    Bytes(ChunkAddress),
    ///
    Register(RegisterAddress),
    ///
    Spentbook(SpentbookAddress),
}

impl DataAddress {
    /// The xorname.
    pub fn name(&self) -> &XorName {
        match self {
            Self::Bytes(address) => address.name(),
            Self::Register(address) => address.name(),
            Self::Spentbook(address) => address.name(),
        }
    }

    ///
    pub fn register(name: XorName, tag: u64) -> Self {
        Self::Register(RegisterAddress::new(name, tag))
    }

    ///
    pub fn bytes(name: XorName) -> Self {
        Self::Bytes(ChunkAddress::new(name))
    }
}
