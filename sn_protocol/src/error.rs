// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    storage::{ChunkAddress, DbcAddress, RecordKind, RegisterAddress},
    NetworkAddress,
};
use serde::{Deserialize, Serialize};
use sn_dbc::{Hash, SignedSpend};
use thiserror::Error;
use xor_name::XorName;

/// A specialised `Result` type for protocol crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Main error types for the SAFE protocol.
#[derive(Error, Clone, PartialEq, Eq, Serialize, Deserialize, custom_debug::Debug)]
#[non_exhaustive]
pub enum Error {
    #[error("Chunk not found: {0:?}")]
    ChunkNotFound(ChunkAddress),
    #[error("Chunk was not stored, xorname: {0:?}")]
    ChunkNotStored(XorName),

    #[error("Register was not stored, xorname: {0:?}")]
    RegisterNotStored(XorName),
    #[error("Register not found: {0:?}")]
    RegisterNotFound(RegisterAddress),
    #[error("Register operation was not stored: {0:?}")]
    RegisterCmdNotStored(RegisterAddress),
    #[error("Register is Invalid: {0:?}")]
    InvalidRegister(RegisterAddress),
    #[error("Register is Invalid: {0}")]
    RegisterError(#[from] sn_registers::Error),
    #[error("The Register was already created by another owner: {0:?}")]
    RegisterAlreadyClaimed(bls::PublicKey),

    /// The amount paid by payment proof is not the required for the received content
    #[error("The amount paid by payment proof is not the required for the received content, paid {paid}, expected {expected}")]
    PaymentProofInsufficientAmount { paid: usize, expected: usize },
    /// At least one input of payment proof provided has a mismatching spend Tx
    #[error("At least one input of payment proof provided for {0:?} has a mismatching spend Tx")]
    PaymentProofTxMismatch(XorName),
    /// Payment proof received has no inputs
    #[error("Payment proof received for {0:?} has no inputs in its transaction")]
    PaymentProofWithoutInputs(XorName),
    /// The id of the fee output found in a storage payment proof is invalid
    #[error("The id of the fee output found in a storage payment proof is invalid: {}", .0.to_hex())]
    PaymentProofInvalidFeeOutput(Hash),

    /// Cannot add another entry since the register entry cap has been reached.
    #[error("Cannot add another entry since the register entry cap has been reached: {0}")]
    TooManyEntries(usize),
    /// Data authority provided is invalid.
    #[error("Provided PublicKey could not validate signature: {0:?}")]
    InvalidSignature(bls::PublicKey),

    /// Received a empty `Vec<SignedSpend>`
    #[error("Operation received no SignedSpends")]
    MinNumberOfSpendsNotMet,
    /// Received a `Vec<SignedSpend>` with more than two spends
    #[error("Incoming SpendDbc PUT with incorrect number of SignedSpend")]
    MaxNumberOfSpendsExceeded,
    #[error("Spend not found: {0:?}")]
    SpendNotFound(DbcAddress),
    #[error("Failed to store spend: {0:?}")]
    SpendNotStored(String),
    #[error("Insufficient valid spends found: {0:?}")]
    InsufficientValidSpendsFound(DbcAddress),
    #[error("A double spend was detected. Two diverging signed spends: {0:?}, {1:?}")]
    DoubleSpendAttempt(Box<SignedSpend>, Box<SignedSpend>),
    /// Cannot verify a Spend signature.
    #[error("Spend signature is invalid: {0}")]
    InvalidSpendSignature(String),
    #[error("Spend parents are invalid: {0}")]
    InvalidSpendParents(String),
    #[error("Invalid Parent Tx: {0}")]
    InvalidParentTx(String),
    /// One or more parent spends of a requested spend has an invalid hash
    #[error("Invalid parent spend hash: {0}")]
    BadParentSpendHash(String),

    /// Replication not found.
    #[error("Peer {holder:?} cannot find ReplicatedData {address:?}")]
    ReplicatedDataNotFound {
        /// Holder that being contacted
        holder: NetworkAddress,
        /// Address of the missing data
        address: NetworkAddress,
    },

    /// Payment proof provided deemed invalid
    #[error("Payment proof provided deemed invalid for item's name {addr_name:?}: {reason}")]
    InvalidPaymentProof {
        /// XorName the payment proof deemed invalid for
        addr_name: XorName,
        /// Reason why the payment proof was deemed invalid
        reason: String,
    },

    // Could not Serialize/Deserialize RecordHeader from Record
    #[error("Could not Serialize/Deserialize RecordHeader to/from Record")]
    RecordHeaderParsingFailed,
    // Could not Serialize/Deserialize Record
    #[error("Could not Serialize/Deserialize Record")]
    RecordParsingFailed,
    // The Record::key must match with the one that is derived from the Record::value
    #[error("The Record::key does not match with the key derived from Record::value")]
    RecordKeyMismatch,
    // The RecordKind that was obtained did not match with the expected one
    #[error("The RecordKind obtained from the Record did not match with the expected kind: {0}")]
    RecordKindMismatch(RecordKind),
}
