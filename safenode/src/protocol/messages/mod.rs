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

use super::types::chunk::Chunk;

use serde::{Deserialize, Serialize};
use std::fmt::Debug;

/// Send a request to other peers in the network
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Request {
    /// A (read-only) query sent to nodes.
    Query(Query),
    /// Messages that lead to mutation.
    Cmd(Cmd),
}

/// Respond to other peers in the network
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Response {
    /// The response to a query.
    Query(QueryResponse),
    /// The response to a cmd.
    Cmd(CmdResponse),
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
