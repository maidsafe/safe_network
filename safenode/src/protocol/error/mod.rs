// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod storage;
mod transfer;

pub use storage::StorageError;
pub use transfer::TransferError;

use serde::{Deserialize, Serialize};
use std::{fmt::Debug, result};
use thiserror::Error;

/// A specialised `Result` type for protocol crate.
pub type Result<T> = result::Result<T, Error>;

/// Main error types for the SAFE protocol.
#[derive(Error, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Error {
    /// Storage error.
    #[error("Storage error {0:?}")]
    Storage(#[from] StorageError),
    /// Errors in node transfer handling.
    #[error("Transfer error: {0:?}")]
    Transfers(#[from] TransferError),
}
