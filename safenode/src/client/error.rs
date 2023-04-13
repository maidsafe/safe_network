// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

pub(super) type Result<T, E = Error> = std::result::Result<T, E>;

use crate::protocol::types::register::{Entry, EntryHash};

use std::collections::BTreeSet;
use thiserror::Error;

/// Internal error.
#[derive(Debug, Error)]
#[allow(missing_docs)]
pub enum Error {
    #[error("Network Error {0}.")]
    Network(#[from] crate::network::Error),

    #[error("Protocol error {0}.")]
    Protocol(#[from] crate::protocol::types::error::Error),

    #[error("Events receiver error {0}.")]
    EventsReceiver(#[from] tokio::sync::broadcast::error::RecvError),

    #[error("ResponseTimeout.")]
    ResponseTimeout(#[from] tokio::time::error::Elapsed),

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
