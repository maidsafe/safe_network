// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

#[allow(unused_imports)] // needed by rustdocs links
use super::RegisterQuery;
use crate::{error::Result, messages::ReplicatedData, storage::Chunk, NetworkAddress};
use serde::{Deserialize, Serialize};
use sn_dbc::SignedSpend;
use std::{collections::BTreeSet, fmt::Debug};

use sn_registers::{Entry, EntryHash, Permissions, Register, User};

/// The response to a query, containing the query result.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, custom_debug::Debug)]
pub enum QueryResponse {
    /// If the queried node has validated a corresponding spend
    /// request, it will return the SignedSpend.
    /// It is up to the Client to get this SignedSpend from enough
    /// nodes as to consider it a valid spend. The specific rules
    /// on how many nodes are enough, are found here: (TODO).
    ///
    /// Response to [`GetDbcSpend`]
    ///
    /// [`GetDbcSpend`]: crate::messages::Query::GetSpend
    GetDbcSpend(Result<SignedSpend>),
    //
    // ===== Chunk =====
    //
    /// Response to [`GetChunk`]
    ///
    /// [`GetChunk`]: crate::messages::Query::GetChunk
    GetChunk(Result<Chunk>),
    //
    // ===== ReplicatedData =====
    //
    /// Response to [`GetReplicatedData`]
    ///
    /// [`GetReplicatedData`]: crate::messages::Query::GetReplicatedData
    GetReplicatedData(Result<(NetworkAddress, ReplicatedData)>),
    //
    // ===== Register Data =====
    //
    /// Response to [`RegisterQuery::Get`].
    GetRegister(Result<Register>),
    /// Response to [`RegisterQuery::GetEntry`].
    GetRegisterEntry(Result<Entry>),
    /// Response to [`RegisterQuery::GetOwner`].
    GetRegisterOwner(Result<User>),
    /// Response to [`RegisterQuery::Read`].
    ReadRegister(Result<BTreeSet<(EntryHash, Entry)>>),
    /// Response to [`RegisterQuery::GetPermissions`].
    GetRegisterPermissions(Result<Permissions>),
}

/// The response to a Cmd, containing the query result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CmdResponse {
    //
    // ===== Dbc Spends =====
    //
    /// Response to DbcCmd::Spend.
    Spend(Result<CmdOk>),
    //
    // ===== Register Data =====
    //
    /// Response to RegisterCmd::Create.
    CreateRegister(Result<()>),
    /// Response to RegisterCmd::Edit.
    EditRegister(Result<()>),
    //
    // ===== Replication =====
    //
    /// Response to replication cmd
    Replicate(Result<()>),
}

/// The Ok variant of a CmdResponse
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CmdOk {
    StoredSuccessfully,
    DataAlreadyPresent,
}

impl std::fmt::Display for QueryResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QueryResponse::GetDbcSpend(Ok(signed_spend)) => {
                write!(f, "GetDbcSpend(Ok({:?}))", signed_spend.dbc_id())
            }
            _ => write!(f, "{:?}", self),
        }
    }
}
