// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

pub(crate) type Result<T> = std::result::Result<T, Error>;

use super::ClientEvent;
use sn_registers::{Entry, EntryHash};
use std::collections::BTreeSet;
use thiserror::Error;

/// Internal error.
#[derive(Debug, Error)]
#[allow(missing_docs)]
pub enum Error {
    #[error("Genesis error {0}")]
    GenesisError(#[from] sn_transfers::genesis::Error),

    /// Could not acquire a Semaphore permit.
    #[error("Could not acquire a Semaphore permit.")]
    CouldNotAcquireSemaphorePermit(#[from] tokio::sync::AcquireError),

    /// Could not acquire a network semaphore
    #[error("Network layer does not have the expected concurrency limiter.")]
    NoNetworkConcurrencyLimiterFound,

    #[error("Transfer Error {0}.")]
    Transfers(#[from] sn_transfers::wallet::Error),

    #[error("Network Error {0}.")]
    Network(#[from] sn_networking::Error),

    #[error("Protocol error {0}.")]
    Protocol(#[from] sn_protocol::error::Error),

    #[error("Register error {0}.")]
    Register(#[from] sn_registers::Error),

    #[error("Events receiver error {0}.")]
    EventsReceiver(#[from] tokio::sync::broadcast::error::RecvError),

    #[error("Events sender error {0}.")]
    EventsSender(#[from] tokio::sync::broadcast::error::SendError<ClientEvent>),

    /// A general error when verifying a transfer validity in the network.
    #[error("Failed to verify transfer validity in the network {0}")]
    CouldNotVerifyTransfer(String),

    #[error("Chunks error {0}.")]
    Chunks(#[from] super::chunks::Error),

    #[error(
        "Content branches detected in the Register which need to be merged/resolved by user. \
        Entries hashes of branches are: {0:?}"
    )]
    ContentBranchDetected(BTreeSet<(EntryHash, Entry)>),

    /// File system access error.
    #[error("System IO Error {0}.")]
    SystemIO(#[from] std::io::Error),

    /// SelfEncryption error.
    #[error("SelfEncryption Error {0}.")]
    SelfEncryptionIO(#[from] self_encryption::Error),
}
