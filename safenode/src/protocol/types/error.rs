// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{
    address::{ChunkAddress, DbcAddress, RegisterAddress},
    authority::PublicKey,
    register::{EntryHash, User},
};
use serde::{Deserialize, Serialize};
use sn_dbc::SignedSpend;
use std::{collections::BTreeSet, fmt::Debug, result};
use thiserror::Error;

/// A specialised `Result` type for types crate.
pub type Result<T> = result::Result<T, Error>;

/// Main error type for the crate.
#[derive(Error, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Error {
    /// Access denied for user
    #[error("Access denied for user: {0:?}")]
    AccessDenied(User),
    /// Serialization error
    #[error("Serialisation error: {0}")]
    Serialisation(String),
    /// Chunk not found.
    #[error("Chunk not found: {0:?}")]
    ChunkNotFound(ChunkAddress),
    /// Register not found.
    #[error("Register not found: {0:?}")]
    RegisterNotFound(RegisterAddress),
    /// Spend not found.
    #[error("Spend not found: {0:?}")]
    SpendNotFound(DbcAddress),
    /// A double spend attempt was detected.
    #[error("A double spend attempt was detected. Set of mismatching spends: {0:?}")]
    DoubleSpendAttempt(BTreeSet<SignedSpend>),
    /// A parent spend of a requested spend could not be confirmed as valid.
    #[error(
        "A parent tx of a requested spend could not be confirmed as valid. All invalid parents' addresses {0:?}"
    )]
    InvalidSpendParentTx(BTreeSet<DbcAddress>),
    /// Unexpected responses.
    #[error("Unexpected responses")]
    UnexpectedResponses,
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
    #[error("Provided PublicKey could not validate signature {0:?}")]
    InvalidSignature(PublicKey),
    /// The CRDT operation cannot be applied as it targets a different content address.
    #[error("The CRDT operation cannot be applied as it targets a different content address.")]
    CrdtWrongAddress(RegisterAddress),
    /// Failed to write file, likely due to a system Io error
    #[error("Failed to write file")]
    FailedToWriteFile,
    /// Not enough space to store the value.
    #[error("Not enough space")]
    NotEnoughSpace,
    /// Bincode error.
    #[error("Bincode error:: {0}")]
    Bincode(String),
    /// I/O error.
    #[error("I/O error: {0}")]
    Io(String),
    /// Hex decoding error.
    #[error("Hex decoding error:: {0}")]
    HexDecoding(String),
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
}
