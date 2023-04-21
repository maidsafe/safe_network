// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{
    register::{EntryHash, User},
    ChunkAddress, DbcAddress, RegisterAddress,
};

use sn_dbc::{Error as DbcError, SignedSpend};

use serde::{Deserialize, Serialize};
use std::{fmt::Debug, path::PathBuf, result};
use thiserror::Error;

/// A specialised `Result` type for protocol crate.
pub type Result<T> = result::Result<T, Error>;

/// Main error type for the crate.
#[derive(Error, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Error {
    /// Chunk not found.
    #[error("Chunk not found: {0:?}")]
    ChunkNotFound(ChunkAddress),
    /// No filename found
    #[error("Path is not a file: {0}")]
    PathIsNotAFile(PathBuf),
    /// Invalid filename
    #[error("Invalid chunk filename: {0}")]
    InvalidFilename(PathBuf),
    /// Register not found.
    #[error("Register not found: {0:?}")]
    RegisterNotFound(RegisterAddress),
    /// Register command/op destination address mistmatch
    #[error(
        "Register command destination address ({cmd_dst_addr:?}) \
         doesn't match stored Register address: {reg_addr:?}"
    )]
    RegisterAddrMismatch {
        /// Register command destination address
        cmd_dst_addr: RegisterAddress,
        /// Stored Register address
        reg_addr: RegisterAddress,
    },
    /// Access denied for user
    #[error("Access denied for user: {0:?}")]
    AccessDenied(User),
    /// Entry is too big to fit inside a register
    #[error("Entry is too big to fit inside a register: {size}, max: {max}")]
    EntryTooBig {
        /// Size of the entry
        size: usize,
        /// Maximum entry size allowed
        max: usize,
    },
    /// Cannot add another entry since the register entry cap has been reached.
    #[error("Cannot add another entry since the register entry cap has been reached: {0}")]
    TooManyEntries(usize),
    /// Entry could not be found on the data
    #[error("Requested entry not found {0}")]
    NoSuchEntry(EntryHash),
    /// User entry could not be found on the data
    #[error("Requested user not found {0:?}")]
    NoSuchUser(User),
    /// The CRDT operation cannot be applied as it targets a different content address.
    #[error("The CRDT operation cannot be applied as it targets a different content address.")]
    CrdtWrongAddress(RegisterAddress),
    /// Data authority provided is invalid.
    #[error("Provided PublicKey could not validate signature {0:?}")]
    InvalidSignature(bls::PublicKey),
    /// Spend not found.
    #[error("Spend not found: {0:?}")]
    SpendNotFound(DbcAddress),
    /// A double spend attempt was detected.
    #[error("A double spend attempt was detected. Incoming and existing spend are not the same: {new:?}. Existing: {existing:?}")]
    DoubleSpendAttempt {
        /// New spend that we received.
        new: Box<SignedSpend>,
        /// Existing spend of same id that we already have.
        existing: Box<SignedSpend>,
    },
    /// We were notified about a double spend attempt, but they were for different dbcs.
    #[error("We were notified about a double spend attempt, but they were for different dbcs: {0:?}. Existing: {1:?}")]
    NotADoubleSpendAttempt(Box<SignedSpend>, Box<SignedSpend>),
    /// An error from the `sn_dbc` crate.
    #[error("Dbc error: {0}")]
    Dbcs(String),
    /// Bincode error.
    #[error("Bincode error:: {0}")]
    Bincode(String),
    /// I/O error.
    #[error("I/O error: {0}")]
    Io(String),
    /// Hex decoding error.
    #[error("Hex decoding error:: {0}")]
    HexDecoding(String),
}

impl From<DbcError> for Error {
    fn from(error: DbcError) -> Self {
        Error::Dbcs(error.to_string())
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

impl From<hex::FromHexError> for Error {
    fn from(error: hex::FromHexError) -> Self {
        Self::HexDecoding(error.to_string())
    }
}
