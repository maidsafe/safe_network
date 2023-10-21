// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    storage::{RecordKind, RegisterAddress, SpendAddress},
    NetworkAddress, PrettyPrintRecordKey,
};
use serde::{Deserialize, Serialize};
use sn_transfers::{NanoTokens, SignedSpend};
use thiserror::Error;

/// A specialised `Result` type for protocol crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Main error types for the SAFE protocol.
#[derive(Error, Clone, PartialEq, Eq, Serialize, Deserialize, custom_debug::Debug)]
#[non_exhaustive]
pub enum Error {
    // ---------- record layer + payment errors
    #[error("Record was not stored as no payment supplied: {0:?}")]
    InvalidPutWithoutPayment(PrettyPrintRecordKey),

    /// At this point in replication flows, payment is unimportant and should not be supplied
    #[error("Record should not be a `WithPayment` type: {0:?}")]
    UnexpectedRecordWithPayment(PrettyPrintRecordKey),

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
    SpendNotFound(SpendAddress),
    #[error("Failed to store spend: {0:?}")]
    SpendNotStored(String),
    #[error("A double spend was detected. Two diverging signed spends: {0:?}, {1:?}")]
    DoubleSpendAttempt(Box<SignedSpend>, Box<SignedSpend>),
    #[error("Spend signature is invalid: {0}")]
    SpendSignatureInvalid(String),
    #[error("Invalid Parent Tx: {0}")]
    SpendParentTxInvalid(String),
    #[error("CashNote Spend is empty")]
    SpendIsEmpty,

    // ---------- payment errors
    /// Failed to get the storecost from kademlia store
    #[error("There was an error getting the storecost from kademlia store")]
    GetStoreCostFailed,
    /// The amount paid by payment proof is not the required for the received content
    #[error("The amount paid by payment proof is not the required for the received content, paid {paid}, expected {expected}")]
    PaymentProofInsufficientAmount {
        paid: NanoTokens,
        expected: NanoTokens,
    },
    /// Payment proof received has no inputs
    #[error(
        "Payment proof received with record:{0:?}. No payment for our node in its transaction"
    )]
    NoPaymentToOurNode(PrettyPrintRecordKey),
    /// Payments received could not be stored on node's local wallet
    #[error("Payments received could not be stored on node's local wallet: {0}")]
    FailedToStorePaymentIntoNodeWallet(String),

    // ---------- transfer errors
    #[error("Failed to decypher transfer, we probably are not the recipient")]
    FailedToDecypherTransfer,
    #[error("Failed to get transfer parent spend")]
    FailedToGetTransferParentSpend,
    #[error("Transfer is invalid: {0}")]
    InvalidTransfer(String),

    // ---------- replication errors
    /// Replication not found.
    #[error("Peer {holder:?} cannot find Record {key:?}")]
    ReplicatedRecordNotFound {
        /// Holder that being contacted
        holder: Box<NetworkAddress>,
        /// Key of the missing record
        key: Box<NetworkAddress>,
    },

    // ---------- record errors
    #[error("Record was not stored: {0:?}: {1:?}")]
    RecordNotStored(PrettyPrintRecordKey, String),
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
    // The record already exists at this node
    #[error("The record already exists, so do not charge for it: {0:?}")]
    RecordExists(PrettyPrintRecordKey),
}
