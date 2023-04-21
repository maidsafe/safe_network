// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::domain::client_transfers::Error as ClientTransfersError;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Specialisation of `std::Result`.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Wallet errors.
#[derive(Error, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Error {
    /// Failed to create transfer.
    #[error("Client transfer error {0}")]
    CreateTransfer(#[from] ClientTransfersError),
    /// A general error when a transfer fails.
    #[error("Failed to send tokens due to {0}")]
    CouldNotSendTokens(String),
    /// Failed to parse bytes into a bls key.
    #[error("Failed to parse bls key")]
    FailedToParseBlsKey,
    /// Failed to decode a hex string to a key.
    #[error("Could not decode hex string to key.")]
    FailedToDecodeHexToKey,
    /// Failed to serialize a main key to hex.
    #[error("Could not serialize main key to hex: {0}")]
    FailedToHexEncodeKey(String),
    /// Bls error.
    #[error("Bls error: {0}")]
    Bls(String),
    /// Bincode error.
    #[error("Bincode error:: {0}")]
    Bincode(String),
    /// I/O error.
    #[error("I/O error: {0}")]
    Io(String),
}

impl From<bls::error::Error> for Error {
    fn from(error: bls::error::Error) -> Self {
        Error::Bls(error.to_string())
    }
}

impl From<bincode::Error> for Error {
    fn from(error: bincode::Error) -> Self {
        Self::Bincode(error.to_string())
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

// impl From<hex::FromHexError> for Error {
//     fn from(error: hex::FromHexError) -> Self {
//         Self::HexDecoding(error.to_string())
//     }
// }
