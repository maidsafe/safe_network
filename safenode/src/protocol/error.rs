// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::domain::{node_transfers::Error as TransferError, storage::Error as StorageError};

use serde::{Deserialize, Serialize};
use std::{fmt::Debug, result};
use thiserror::Error;
use xor_name::XorName;

/// A specialised `Result` type for protocol crate.
pub type Result<T> = result::Result<T, Error>;

/// Main error type for the crate.
#[derive(Error, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Error {
    /// There was an internal error while processing the request.
    #[error("There was an internal error while processing the request")]
    InternalProcessing(String),
    /// We failed to store chunk as Record
    #[error("Chunk was not stored as Record w/ xorname {0:?}")]
    ChunkNotStored(XorName),
    /// We failed to retrieve data from our local record storage
    #[error("Provider record was not found locally")]
    RecordNotFound,
    /// Storage error.
    #[error("Storage error {0:?}")]
    Storage(#[from] StorageError),
    /// Errors in node transfer handling.
    #[error("TransferError: {0:?}")]
    Transfers(#[from] TransferError),
    /// An error from the sn_dbc crate.
    #[error("Dbc Error {0}")]
    Dbc(String),
    /// Unexpected responses.
    #[error("Unexpected responses")]
    UnexpectedResponses,
    /// Bincode error.
    #[error("Bincode error:: {0}")]
    Bincode(String),
    /// I/O error.
    #[error("I/O error: {0}")]
    Io(String),
}
