// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

//! Data messages and their possible responses.
mod cmd;
mod event;
mod query;
mod register;
mod response;
mod spend;

pub use self::{
    cmd::Cmd,
    event::Event,
    query::Query,
    register::{
        CreateRegister, EditRegister, RegisterCmd, RegisterQuery, ReplicatedRegisterLog,
        SignedRegisterCreate, SignedRegisterEdit,
    },
    response::{CmdResponse, QueryResponse},
    spend::SpendQuery,
};

use super::types::{
    address::{dbc_address, dbc_name, DataAddress, DbcAddress},
    chunk::Chunk,
};

use sn_dbc::SignedSpend;

use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fmt::Debug};
use xor_name::XorName;

/// Send a request to other peers in the network
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Request {
    /// A cmd sent to peers. Cmds are writes, i.e. can cause mutation.
    Cmd(Cmd),
    /// A query sent to peers. Queries are read-only.
    Query(Query),
    /// A fact sent to peers.
    Event(Event),
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
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize)]
pub enum ReplicatedData {
    /// A chunk of data.
    Chunk(Chunk),
    /// A single cmd for a register.
    RegisterWrite(RegisterCmd),
    /// An entire op log of a register.
    RegisterLog(ReplicatedRegisterLog),
    /// A valid spend.
    ValidSpend(SignedSpend),
    /// A dbc marked as having attempted double spend.
    DoubleSpend((DbcAddress, BTreeSet<SignedSpend>)),
}

impl Request {
    /// Used to send a request to the close group of the address.
    pub fn dst(&self) -> DataAddress {
        match self {
            Request::Cmd(cmd) => cmd.dst(),
            Request::Query(query) => query.dst(),
            Request::Event(event) => event.dst(),
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
            Self::ValidSpend(spend) => dbc_name(spend.dbc_id()),
            Self::DoubleSpend((address, _)) => *address.name(),
        }
    }

    /// Return the dst.
    pub fn dst(&self) -> DataAddress {
        match self {
            Self::Chunk(chunk) => DataAddress::Chunk(*chunk.address()),
            Self::RegisterLog(log) => DataAddress::Register(log.address),
            Self::RegisterWrite(cmd) => DataAddress::Register(cmd.dst()),
            Self::ValidSpend(spend) => DataAddress::Spend(dbc_address(spend.dbc_id())),
            Self::DoubleSpend((address, _)) => DataAddress::Spend(*address),
        }
    }
}
