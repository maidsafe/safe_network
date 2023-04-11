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
    /// One or more parent spends of a requested spend had a different dst tx hash than the signed spend src tx hash.
    #[error(
        "The signed spend src tx ({signed_src_tx_hash:?}) did not match the provided source tx's hash: {provided_src_tx_hash:?}"
    )]
    SignedSrcTxHashDoesNotMatchProvidedSrcTxHash {
        /// The signed spend src tx hash.
        signed_src_tx_hash: sn_dbc::Hash,
        /// The hash of the provided source tx.
        provided_src_tx_hash: sn_dbc::Hash,
    },
    /// One or more parent spends of a requested spend had a different dst tx hash than the signed spend src tx hash.
    #[error(
        "The signed spend src tx ({signed_src_tx_hash:?}) did not match a valid parent's dst tx hash: {parent_dst_tx_hash:?}"
    )]
    SpendSrcTxHashParentTxHashMismatch {
        /// The signed spend src tx hash.
        signed_src_tx_hash: sn_dbc::Hash,
        /// The dst hash of a parent signed spend.
        parent_dst_tx_hash: sn_dbc::Hash,
    },
    /// The provided source tx did not check out when verified with all supposed inputs to it (i.e. our spends parents).
    #[error(
        "The provided source tx (with hash {provided_src_tx_hash:?}) when verified with all supposed inputs to it (i.e. our spends parents).."
    )]
    InvalidSourceTxProvided {
        /// The signed spend src tx hash.
        signed_src_tx_hash: sn_dbc::Hash,
        /// The hash of the provided source tx.
        provided_src_tx_hash: sn_dbc::Hash,
    },
    /// One or more parent spends of a requested spend could not be confirmed as valid.
    /// The full set of parents checked are contained in this error.
    #[error(
        "A parent tx of a requested spend could not be confirmed as valid. All parent signed spends of that tx {0:?}"
    )]
    InvalidSpendParent(BTreeSet<Box<SignedSpend>>),
    /// An error from the sn_dbc crate.
    #[error("Dbc Error {0}")]
    Dbc(String),
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
