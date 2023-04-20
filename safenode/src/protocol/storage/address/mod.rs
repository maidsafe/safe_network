// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod chunk;
mod dbc;
mod register;

pub use self::{
    chunk::ChunkAddress,
    dbc::{dbc_address, dbc_name, DbcAddress},
    register::RegisterAddress,
};

use sn_dbc::DbcId;

use serde::{Deserialize, Serialize};
use xor_name::XorName;

/// An address of data on the network.
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Debug)]
pub enum DataAddress {
    ///
    Chunk(ChunkAddress),
    ///
    Register(RegisterAddress),
    ///
    Spend(DbcAddress),
}

impl DataAddress {
    /// The xorname.
    pub fn name(&self) -> &XorName {
        match self {
            Self::Chunk(address) => address.name(),
            Self::Register(address) => address.name(),
            Self::Spend(address) => address.name(),
        }
    }

    ///
    pub fn chunk(name: XorName) -> Self {
        Self::Chunk(ChunkAddress::new(name))
    }

    ///
    pub fn register(name: XorName, tag: u64) -> Self {
        Self::Register(RegisterAddress::new(name, tag))
    }

    ///
    pub fn spend(dbc_id: DbcId) -> Self {
        Self::Spend(DbcAddress::new(dbc_name(&dbc_id)))
    }
}
