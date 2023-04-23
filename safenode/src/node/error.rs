// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    domain::{dbc_genesis::GenesisError, node_transfers::Error as TransferError},
    network::Error as NetworkError,
    protocol::error::Error as ProtocolError,
};

use thiserror::Error;

pub(super) type Result<T, E = Error> = std::result::Result<T, E>;

/// Internal error.
#[derive(Debug, Error)]
#[allow(missing_docs)]
pub enum Error {
    #[error("Network error {0}")]
    Network(#[from] NetworkError),

    #[error("Protocol error {0}")]
    Protocol(#[from] ProtocolError),

    /// Errors in node transfer handling.
    #[error("TransferError: {0:?}")]
    Transfers(#[from] TransferError),

    #[error("Node wallet load issue: {0}.")]
    CouldNotLoadWallet(String),

    #[error("Genesis error {0}")]
    Genesis(#[from] GenesisError),

    #[error("ResponseTimeout")]
    ResponseTimeout(#[from] tokio::time::error::Elapsed),
}
