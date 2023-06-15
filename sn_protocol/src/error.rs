// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    storage::{
        registers::{EntryHash, User},
        ChunkAddress, DbcAddress, RegisterAddress,
    },
    NetworkAddress,
};

use serde::{Deserialize, Serialize};
use sn_dbc::SignedSpend;
use thiserror::Error;
use xor_name::XorName;

/// A specialised `Result` type for protocol crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Main error types for the SAFE protocol.
#[derive(Error, Clone, PartialEq, Eq, Serialize, Deserialize, custom_debug::Debug)]
#[non_exhaustive]
pub enum Error {
    /// Chunk not found.
    #[error("Chunk not found: {0:?}")]
    ChunkNotFound(ChunkAddress),
    /// Replication not found.
    #[error("Peer {holder:?} cann't find ReplicatedData {address:?}")]
    ReplicatedDataNotFound {
        /// Holder that being contacted
        holder: NetworkAddress,
        /// Address of the missing data
        address: NetworkAddress,
    },
    /// We failed to store chunk
    #[error("Chunk was not stored w/ xorname {0:?}")]
    ChunkNotStored(XorName),
    /// Register not found.
    #[error("Register not found: {0:?}")]
    RegisterNotFound(RegisterAddress),
    /// Register operation was not stored.
    #[error("Register operation was not stored: {0:?}")]
    RegisterCmdNotStored(RegisterAddress),
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
    /// Payment proof provided deemed invalid
    #[error("Payment proof provided deemed invalid for item's name {addr_name:?}: {reason}")]
    InvalidPaymentProof {
        /// XorName the payment proof deemed invalid for
        addr_name: XorName,
        /// Reason why the payment proof was deemed invalid
        reason: String,
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
    /// Insufficient valid spends found to make it valid, less that majority of closest peers
    #[error("Insufficient valid spends found: {0:?}")]
    InsufficientValidSpendsFound(DbcAddress),
    /// Node failed to store spend
    #[error("Failed to store spend: {0}")]
    FailedToStoreSpend(String),
    /// Node failed to get spend
    #[error("Failed to get spend: {0:?}")]
    FailedToGetSpend(DbcAddress),
    /// A double spend was detected.
    #[error("A double spend was detected. Two diverging signed spends: {0:?}, {1:?}")]
    DoubleSpendAttempt(Box<SignedSpend>, Box<SignedSpend>),
    /// Cannot verify a Spend signature.
    #[error("Spend signature is invalid: {0}")]
    InvalidSpendSignature(String),
    /// Cannot verify a Spend's parents.
    #[error("Spend parents are invalid: {0}")]
    InvalidSpendParents(String),

    /// The DBC we're trying to Spend came with an invalid parent Tx
    #[error("Invalid Parent Tx: {0}")]
    InvalidParentTx(String),
    /// One or more parent spends of a requested spend has an invalid hash
    #[error("Invalid parent spend hash: {0}")]
    BadParentSpendHash(String),
    /// The provided source tx did not check out when verified with all supposed inputs to it (i.e. our spends parents).
    #[error("The provided source tx is invalid: {0}")]
    InvalidSourceTxProvided(String),
}
