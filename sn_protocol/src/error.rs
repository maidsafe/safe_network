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
use sn_dbc::{SignedSpend, Token};
use thiserror::Error;
use xor_name::XorName;

/// A specialised `Result` type for protocol crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Main error types for the SAFE protocol.
#[derive(Error, Clone, PartialEq, Eq, Serialize, Deserialize, custom_debug::Debug)]
#[non_exhaustive]
pub enum Error {
    // ---------- chunk errors
    #[error("Chunk not found: {0:?}")]
    ChunkNotFound(ChunkAddress),
    #[error("Chunk was not stored, xorname: {0:?}")]
    ChunkNotStored(XorName),

    // ---------- register errors
    #[error("Register was not stored: {0}")]
    RegisterNotStored(Box<RegisterAddress>),
    #[error("Register not found: {0}")]
    RegisterNotFound(Box<RegisterAddress>),
    #[error("Register is Invalid: {0}")]
    RegisterInvalid(Box<RegisterAddress>),
    #[error("Register is Invalid: {0}")]
    RegisterError(#[from] sn_registers::Error),
    #[error("The Register was already created by another owner: {0:?}")]
    RegisterAlreadyClaimed(bls::PublicKey),

    // ---------- spend errors
    #[error("Spend not found: {0:?}")]
    SpendNotFound(DbcAddress),
    #[error("Failed to store spend: {0:?}")]
    SpendNotStored(String),
    #[error("A double spend was detected. Two diverging signed spends: {0:?}, {1:?}")]
    DoubleSpendAttempt(Box<SignedSpend>, Box<SignedSpend>),
    #[error("Spend signature is invalid: {0}")]
    SpendSignatureInvalid(String),
    #[error("Invalid Parent Tx: {0}")]
    SpendParentTxInvalid(String),
    #[error("Dbc Spend is empty")]
    SpendIsEmpty,

    // ---------- payment errors
    /// Failed to get the storecost from kademlia store
    #[error("There was an error getting the storecost from kademlia store")]
    GetStoreCostFailed,
    #[error("There was an error signing the storecost from kademlia store")]
    SignStoreCostFailed,
    /// The amount paid by payment proof is not the required for the received content
    #[error("The amount paid by payment proof is not the required for the received content, paid {paid}, expected {expected}")]
    PaymentProofInsufficientAmount { paid: Token, expected: Token },
    /// At least one input of payment proof provided has a mismatching spend Tx
    #[error("At least one input of payment proof provided for {0:?} has a mismatching spend Tx")]
    PaymentProofTxMismatch(XorName),
    /// Payment proof received has no inputs
    #[error("Payment proof received for {0:?} has no dbc for this node in its transaction")]
    NoPaymentToThisNode(XorName),
    /// The id of the fee output found in a storage payment proof is invalid
    #[error("The id of the fee output found in a storage payment proof is invalid: {0:?}")]
    PaymentProofInvalidFee(Token),
    /// Payment proof provided deemed invalid
    #[error("Payment proof provided deemed invalid for item's name {addr_name:?}: {reason}")]
    InvalidPaymentProof {
        /// XorName the payment proof deemed invalid for
        addr_name: XorName,
        /// Reason why the payment proof was deemed invalid
        reason: String,
    },
    #[error("UTXO serialisation failed")]
    UtxoSerialisationFailed,
    #[error("UTXO decryption failed")]
    UtxoDecryptionFailed,

    // ---------- replication errors
    /// Replication not found.
    #[error("Peer {holder:?} cannot find ReplicatedData {address:?}")]
    ReplicatedDataNotFound {
        /// Holder that being contacted
        holder: Box<NetworkAddress>,
        /// Address of the missing data
        address: Box<NetworkAddress>,
    },

    // ---------- record errors
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
