// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

//! Data messages and their possible responses.
mod cmd;
mod query;
mod register;
mod response;

pub use self::{
    cmd::Cmd,
    query::Query,
    register::{
        CreateRegister, EditRegister, RegisterCmd, RegisterQuery, ReplicatedRegisterLog,
        SignedRegisterCreate, SignedRegisterEdit,
    },
    response::{CmdResponse, QueryResponse},
};

use super::types::{address::DataAddress, chunk::Chunk};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use xor_name::XorName;

/// Send a request to other peers in the network
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Request {
    /// Messages that lead to mutation.
    Cmd(Cmd),
    /// A (read-only) query sent to nodes.
    Query(Query),
}

/// Respond to other peers in the network
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Response {
    /// The response to a cmd.
    Cmd(CmdResponse),
    /// The response to a query.
    Query(QueryResponse),
}

/// Messages to replicated data among nodes on the network
#[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize)]
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
    pub fn dst(&self) -> DataAddress {
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
    pub fn dst(&self) -> DataAddress {
        match self {
            Self::Chunk(chunk) => DataAddress::Chunk(*chunk.address()),
            Self::RegisterLog(log) => DataAddress::Register(log.address),
            Self::RegisterWrite(cmd) => DataAddress::Register(cmd.dst()),
        }
    }
}
