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

use crate::storage::Chunk;

pub use self::{
    cmd::Cmd,
    node_id::NodeId,
    query::Query,
    register::{
        CreateRegister, EditRegister, RegisterCmd, RegisterQuery, ReplicatedRegisterLog,
        SignedRegisterCreate, SignedRegisterEdit,
    },
    response::{CmdResponse, QueryResponse},
};

use super::NetworkAddress;

use serde::{Deserialize, Serialize};
use xor_name::XorName;

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
    Chunk(Chunk),
    /// A single cmd for a register.
    RegisterWrite(RegisterCmd),
    /// An entire op log of a register.
    RegisterLog(ReplicatedRegisterLog),
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
    pub fn name(&self) -> XorName {
        match self {
            Self::Chunk(chunk) => *chunk.name(),
            Self::RegisterLog(log) => *log.address.name(),
            Self::RegisterWrite(cmd) => *cmd.dst().name(),
        }
    }

    /// Return the dst.
    pub fn dst(&self) -> NetworkAddress {
        match self {
            Self::Chunk(chunk) => NetworkAddress::from_chunk_address(*chunk.address()),
            Self::RegisterLog(log) => NetworkAddress::from_register_address(log.address),
            Self::RegisterWrite(cmd) => NetworkAddress::from_register_address(cmd.dst()),
        }
    }
}

impl std::fmt::Display for Response {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Response::Query(QueryResponse::GetDbcSpend(Ok(spend))) => {
                write!(f, "GetDbcSpend(Ok({:?}))", spend.dbc_id())
            }
            _ => write!(f, "{:?}", self),
        }
    }
}
