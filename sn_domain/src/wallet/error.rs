// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use thiserror::Error;

/// Specialisation of `std::Result`.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Transfer errors.
#[derive(Debug, Error)]
pub enum Error {
    /// Failed to create transfer.
    #[error("Transfer error {0}")]
    CreateTransfer(#[from] crate::client_transfers::Error),
    /// A general error when a transfer fails.
    #[error("Failed to send tokens due to {0}")]
    CouldNotSendTokens(String),
    /// A general error when verifying a transfer validity in the network.
    #[error("Failed to verify transfer validity in the network {0}")]
    CouldNotVerifyTransfer(String),
    /// Failed to parse bytes into a bls key.
    #[error("Failed to parse bls key")]
    FailedToParseBlsKey,
    /// Failed to decode a hex string to a key.
    #[error("Could not decode hex string to key.")]
    FailedToDecodeHexToKey,
    /// Failed to serialize a main key to hex.
    #[error("Could not serialize main key to hex: {0}")]
    FailedToHexEncodeKey(String),
    /// Dbc error.
    #[error("Dbc error: {0}")]
    Dbc(#[from] Box<sn_dbc::Error>),
    /// Bls error.
    #[error("Bls error: {0}")]
    Bls(#[from] bls::error::Error),
    /// Bincode error.
    #[error("Bincode error:: {0}")]
    Bincode(#[from] bincode::Error),
    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
