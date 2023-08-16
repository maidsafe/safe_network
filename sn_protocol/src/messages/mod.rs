// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

//! Data messages and their possible responses.
mod cmd;
mod node_id;
mod query;
mod register;
mod response;
mod utxo;

pub use self::{
    cmd::{Cmd, Hash, MerkleTreeNodesType, PaymentTransactions},
    node_id::NodeId,
    query::Query,
    register::RegisterCmd,
    response::{CmdOk, CmdResponse, QueryResponse},
    utxo::{Transfer, Utxo},
};

use super::NetworkAddress;
use crate::{
    error::{Error, Result},
    storage::{ChunkWithPayment, DbcAddress},
};
use serde::{Deserialize, Serialize};
use sn_dbc::SignedSpend;
use sn_registers::SignedRegister;
use xor_name::XorName;

#[allow(clippy::large_enum_variant)]
/// A request to peers in the network
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Request {
    /// A cmd sent to peers. Cmds are writes, i.e. can cause mutation.
    Cmd(Cmd),
    /// A query sent to peers. Queries are read-only.
    Query(Query),
}

/// A response to peers in the network.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Response {
    /// The response to a cmd.
    Cmd(CmdResponse),
    /// The response to a query.
    Query(QueryResponse),
}

#[derive(custom_debug::Debug, Eq, PartialEq, Clone, Serialize, Deserialize)]
pub enum ReplicatedData {
    /// A chunk of data.
    Chunk(ChunkWithPayment),
    /// A set of SignedSpends
    DbcSpend(Vec<SignedSpend>),
    /// A signed register
    Register(SignedRegister),
}

impl Request {
    /// Used to send a request to the close group of the address.
    pub fn dst(&self) -> NetworkAddress {
        match self {
            Request::Cmd(cmd) => cmd.dst(),
            Request::Query(query) => query.dst(),
        }
    }
}

impl ReplicatedData {
    /// Return the name.
    pub fn name(&self) -> Result<XorName> {
        let name = match self {
            Self::Chunk(chunk) => *chunk.chunk.name(),
            Self::DbcSpend(spends) => {
                if let Some(spend) = spends.first() {
                    *DbcAddress::from_dbc_id(spend.dbc_id()).xorname()
                } else {
                    return Err(Error::SpendIsEmpty);
                }
            }
            Self::Register(register) => register.address().xorname(),
        };
        Ok(name)
    }

    /// Return the dst.
    pub fn dst(&self) -> Result<NetworkAddress> {
        let dst = match self {
            Self::Chunk(chunk) => NetworkAddress::from_chunk_address(*chunk.chunk.address()),
            Self::DbcSpend(spends) => {
                if let Some(spend) = spends.first() {
                    NetworkAddress::from_dbc_address(DbcAddress::from_dbc_id(spend.dbc_id()))
                } else {
                    return Err(Error::SpendIsEmpty);
                }
            }
            Self::Register(register) => NetworkAddress::from_register_address(*register.address()),
        };
        Ok(dst)
    }
}

impl std::fmt::Display for Response {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}
