// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::RegisterCmd;

use crate::protocol::types::{
    address::{dbc_address, ChunkAddress, DataAddress},
    chunk::Chunk,
    fees::FeeCiphers,
};

use sn_dbc::{DbcTransaction, SignedSpend};

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use xor_name::XorName;

/// Data and Dbc cmds - recording spends or creating, updating, and removing data.
///
/// See the [`types`] module documentation for more details of the types supported by the Safe
/// Network, and their semantics.
///
/// [`types`]: crate::protocol::types
#[allow(clippy::large_enum_variant)]
#[derive(Eq, PartialEq, Clone, Serialize, Deserialize, custom_debug::Debug)]
pub enum Cmd {
    /// [`Chunk`] write operation.
    ///
    /// [`Chunk`]: crate::protocol::types::chunk::Chunk
    StoreChunk(Chunk),
    /// [`Register`] write operation.
    ///
    /// [`Register`]: crate::protocol::types::register::Register
    Register(RegisterCmd),
    /// [`SignedSpend`] write operation.
    ///
    /// [`SignedSpend`]: sn_dbc::SignedSpend
    SpendDbc {
        /// The spend to be recorded.
        /// It contains the transaction it is being spent in.
        #[debug(skip)]
        signed_spend: Box<SignedSpend>,
        /// The transaction that this spend was created in.
        #[debug(skip)]
        source_tx: Box<DbcTransaction>,
        /// As to avoid impl separate cmd flow, we send
        /// all fee ciphers to all Nodes for now.
        #[debug(skip)]
        fee_ciphers: BTreeMap<XorName, FeeCiphers>,
    },
}

impl Cmd {
    /// Used to send a cmd to the close group of the address.
    pub fn dst(&self) -> DataAddress {
        match self {
            Cmd::StoreChunk(chunk) => DataAddress::Chunk(ChunkAddress::new(*chunk.name())),
            Cmd::Register(cmd) => DataAddress::Register(cmd.dst()),
            Cmd::SpendDbc { signed_spend, .. } => {
                DataAddress::Spend(dbc_address(signed_spend.dbc_id()))
            }
        }
    }
}
