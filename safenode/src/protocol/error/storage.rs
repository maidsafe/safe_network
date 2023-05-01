// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::protocol::storage::{
    registers::{EntryHash, User},
    ChunkAddress, DbcAddress, RegisterAddress,
};

use sn_dbc::SignedSpend;
use xor_name::XorName;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors relaated to storage operation on the network.
#[derive(Error, Clone, PartialEq, Eq, Serialize, Deserialize, custom_debug::Debug)]
#[non_exhaustive]
pub enum StorageError {
    /// Chunk not found.
    #[error("Chunk not found: {0:?}")]
    ChunkNotFound(ChunkAddress),
    /// We failed to store chunk
    #[error("Chunk was not stored w/ xorname {0:?}")]
    ChunkNotStored(XorName),
    /// Register not found.
    #[error("Register not found: {0:?}")]
    RegisterNotFound(RegisterAddress),
    /// Register operation destination address mistmatch
    #[error(
        "The CRDT operation cannot be applied since the Register operation destination address ({dst_addr:?}) \
         doesn't match the targeted Register's address: {reg_addr:?}"
    )]
    RegisterAddrMismatch {
        /// Register operation destination address
        dst_addr: RegisterAddress,
        /// Targeted Register's address
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
    /// Data authority provided is invalid.
    #[error("Provided PublicKey could not validate signature: {0:?}")]
    InvalidSignature(bls::PublicKey),
    /// Spend not found.
    #[error("Spend not found: {0:?}")]
    SpendNotFound(DbcAddress),
    /// A double spend attempt was detected.
    #[error("A double spend attempt was detected. Incoming and existing spend are not the same: {new:?}. Existing: {existing:?}")]
    DoubleSpendAttempt {
        /// New spend that we received.
        #[debug(skip)]
        new: Box<SignedSpend>,
        /// Existing spend of same id that we already have.
        #[debug(skip)]
        existing: Box<SignedSpend>,
    },
    /// We were notified about a double spend attempt, but they were for different dbcs.
    #[debug(skip)]
    #[error("We were notified about a double spend attempt, but they were for different dbcs. One: {one:?}, another: {other:?}")]
    NotADoubleSpendAttempt {
        /// One of the spends provided.
        #[debug(skip)]
        one: Box<SignedSpend>,
        /// The other spend provided.
        #[debug(skip)]
        other: Box<SignedSpend>,
    },
    /// A spend that was attempted to be added was already marked as double spend.
    #[error("A spend that was attempted to be added was already marked as double spend: {0:?}")]
    AlreadyMarkedAsDoubleSpend(DbcAddress),
    /// NB: This is a temporary error, which circumvents double spend detection for now.
    /// A spend that was attempted to be added already existed.
    #[error("A spend that was attempted to be added already existed: {0:?}")]
    AlreadyExists(DbcAddress),
    /// Cannot verify a Spend signature.
    #[error("Spend signature is invalid: {0}")]
    InvalidSpendSignature(String),
    /// Bincode error.
    // FIXME: remove this variant as it doesn't belong to the protocol
    #[error("Bincode error:: {0}")]
    Bincode(String),
    /// I/O error.
    // FIXME: remove this variant as it doesn't belong to the protocol
    #[error("I/O error: {0}")]
    Io(String),
}

impl From<bincode::Error> for StorageError {
    fn from(error: bincode::Error) -> Self {
        Self::Bincode(error.to_string())
    }
}

impl From<std::io::Error> for StorageError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}
