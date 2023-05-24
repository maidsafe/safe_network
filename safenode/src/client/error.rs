// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

pub(super) type Result<T, E = Error> = std::result::Result<T, E>;

use sn_protocol::storage::registers::{Entry, EntryHash};
use std::collections::BTreeSet;
use thiserror::Error;

use super::ClientEvent;

/// Internal error.
#[derive(Debug, Error)]
#[allow(missing_docs)]
pub enum Error {
    #[error("Network Error {0}.")]
    Network(#[from] sn_networking::Error),

    #[error("Protocol error {0}.")]
    Protocol(#[from] sn_protocol::error::Error),

    #[error("Events receiver error {0}.")]
    EventsReceiver(#[from] tokio::sync::broadcast::error::RecvError),

    #[error("Events sender error {0}.")]
    EventsSender(#[from] tokio::sync::broadcast::error::SendError<ClientEvent>),

    #[error("ResponseTimeout.")]
    ResponseTimeout(#[from] tokio::time::error::Elapsed),

    /// Unexpected responses.
    #[error("Unexpected responses")]
    UnexpectedResponses,

    /// A general error when verifying a transfer validity in the network.
    #[error("Failed to verify transfer validity in the network {0}")]
    CouldNotVerifyTransfer(String),

    #[error("Chunks error {0}.")]
    Chunks(#[from] super::chunks::Error),

    #[error("Serialisation error: {0}")]
    BincodeError(#[from] bincode::Error),

    #[error(
        "Content branches detected in the Register which need to be merged/resolved by user. \
        Entries hashes of branches are: {0:?}"
    )]
    ContentBranchDetected(BTreeSet<(EntryHash, Entry)>),
}
