// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

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

//******************************************
//**** domain/node_transfers/TransferErrors

use crate::protocol::messages::NodeId;

// FIMXE: these should be defined within the protocol rather than from another crate.
use sn_dbc::{DbcId, Error as DbcError, Hash, SignedSpend, Token};

use std::collections::BTreeSet;

/// Transfer errors.
#[derive(Error, custom_debug::Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferError {
    ///
    #[error("The transfer fee is missing.")]
    MissingFee((NodeId, DbcId)),
    ///
    #[error("The transfer feeciphers are missing.")]
    MissingFeeCiphers(NodeId),
    ///
    #[error("Invalid fee blinded amount.")]
    InvalidFeeBlindedAmount,
    ///
    #[error("Too low amount for the transfer fee: {paid}. Min required: {required}.")]
    FeeTooLow {
        ///
        paid: Token,
        ///
        required: Token,
    },
    ///
    #[error(transparent)]
    Fees(#[from] FeeError),
    ///
    #[error("Contacting close group of parent spends failed: {0}.")]
    SpendParentCloseGroupIssue(String),
    ///
    #[error("Fee cipher cecryption failed {0}.")]
    FeeCipherDecryptionFailed(String),
    /// An error from the `sn_dbc` crate.
    #[error("Dbc error: {0}")]
    Dbcs(String),
    /// One or more parent spends of a requested spend had a different dst tx hash than the signed spend src tx hash.
    #[error(
        "The signed spend src tx ({signed_src_tx_hash:?}) did not match the provided source tx's hash: {provided_src_tx_hash:?}"
    )]
    TxSourceMismatch {
        /// The signed spend src tx hash.
        signed_src_tx_hash: Hash,
        /// The hash of the provided source tx.
        provided_src_tx_hash: Hash,
    },
    /// One or more parent spends of a requested spend had a different dst tx hash than the signed spend src tx hash.
    #[error(
        "The signed spend src tx ({signed_src_tx_hash:?}) did not match a valid parent's dst tx hash: {parent_dst_tx_hash:?}. The trail is invalid."
    )]
    TxTrailMismatch {
        /// The signed spend src tx hash.
        signed_src_tx_hash: Hash,
        /// The dst hash of a parent signed spend.
        parent_dst_tx_hash: Hash,
    },
    /// The provided source tx did not check out when verified with all supposed inputs to it (i.e. our spends parents).
    #[error(
        "The provided source tx (with hash {provided_src_tx_hash:?}) when verified with all supposed inputs to it (i.e. our spends parents).."
    )]
    InvalidSourceTxProvided {
        /// The signed spend src tx hash.
        signed_src_tx_hash: Hash,
        /// The hash of the provided source tx.
        provided_src_tx_hash: Hash,
    },
    /// One or more parent spends of a requested spend could not be confirmed as valid.
    /// The full set of parents checked are contained in this error.
    #[debug(skip)]
    #[error(
        "A parent tx of a requested spend could not be confirmed as valid. All parent signed spends of that tx {0:?}"
    )]
    InvalidSpendParent(BTreeSet<Box<SignedSpend>>),
    /// Storage error.
    #[error("Storage error {0:?}")]
    Storage(#[from] StorageError),
}

impl From<DbcError> for TransferError {
    fn from(error: DbcError) -> Self {
        Self::Dbcs(error.to_string())
    }
}

/// Fee errors.
#[derive(Error, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FeeError {
    /// The Node signature over the `RequiredFee` is invalid.
    #[error("Node signature is invalid.")]
    RequiredFeeSignatureInvalid,
    /// Decryption of the amount failed. Wrong key used.
    #[error("Decryption of the amount failed. Wrong key used.")]
    AmountDecryptionFailed,
    /// An error from the `sn_dbc` crate.
    #[error("Dbc error: {0}")]
    Dbcs(String),
}

impl From<DbcError> for FeeError {
    fn from(error: DbcError) -> Self {
        Self::Dbcs(error.to_string())
    }
}

//*******************************************
//**** domain/storage/TransferErrors

// FIXME: do we needd errors that depend on these types??
// do we needs these types in the protocol??
use crate::protocol::storage::{
    registers::{EntryHash, User},
    ChunkAddress, DbcAddress, RegisterAddress,
};

use std::path::PathBuf;

/// Errors relaated to storage operation on the network.
#[derive(Error, Clone, PartialEq, Eq, Serialize, Deserialize, custom_debug::Debug)]
#[non_exhaustive]
pub enum StorageError {
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

impl From<DbcError> for StorageError {
    fn from(error: DbcError) -> Self {
        Self::Dbcs(error.to_string())
    }
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

impl From<hex::FromHexError> for StorageError {
    fn from(error: hex::FromHexError) -> Self {
        Self::HexDecoding(error.to_string())
    }
}
