// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use sn_protocol::PrettyPrintRecordKey;
use sn_transfers::{NanoTokens, WalletError};
use thiserror::Error;

pub(super) type Result<T, E = Error> = std::result::Result<T, E>;

/// Internal error.
#[derive(Debug, Error)]
#[allow(missing_docs)]
pub enum Error {
    #[error("Network error {0}")]
    Network(#[from] sn_networking::Error),

    #[error("Protocol error {0}")]
    Protocol(#[from] sn_protocol::Error),

    #[error("Register error {0}")]
    Register(#[from] sn_registers::Error),

    #[error("WalletError error {0}")]
    Wallet(#[from] WalletError),

    #[error("Transfers Error {0}")]
    Transfers(#[from] sn_transfers::TransferError),

    #[error("Failed to parse NodeEvent")]
    NodeEventParsingFailed,

    #[error("Failed to send a cmd to the node: {0}")]
    NodeCmdFailed(String),

    #[error("Overflow occurred while adding values")]
    NumericOverflow,

    // ---------- Record Errors
    #[error("Record was not stored as no payment supplied: {0:?}")]
    InvalidPutWithoutPayment(PrettyPrintRecordKey<'static>),
    /// At this point in replication flows, payment is unimportant and should not be supplied
    #[error("Record should not be a `WithPayment` type: {0:?}")]
    UnexpectedRecordWithPayment(PrettyPrintRecordKey<'static>),
    // The Record::key must match with the one that is derived from the Record::value
    #[error("The Record::key does not match with the key derived from Record::value")]
    RecordKeyMismatch,

    // ---------- Payment Errors
    #[error("The content of the payment quote is invalid")]
    InvalidQuoteContent,
    #[error("The payment quote's signature is invalid")]
    InvalidQuoteSignature,
    #[error("The payment quote expired")]
    QuoteExpired,
    /// Payment proof received has no inputs
    #[error(
        "Payment proof received with record:{0:?}. No payment for our node in its transaction"
    )]
    NoPaymentToOurNode(PrettyPrintRecordKey<'static>),
    /// Missing network royalties payment
    #[error("Missing network royalties payment in proof received with record: {0:?}.")]
    NoNetworkRoyaltiesPayment(PrettyPrintRecordKey<'static>),
    /// The amount paid by payment proof is not the required for the received content
    #[error("The amount paid by payment proof is not the required for the received content, paid {paid}, expected {expected}")]
    PaymentProofInsufficientAmount {
        paid: NanoTokens,
        expected: NanoTokens,
    },

    // ---------- Initialize Errors
    #[error("Failed to generate a reward key")]
    FailedToGenerateRewardKey,

    // ---------- Miscellaneous Errors
    #[error("Failed to obtain node's current port")]
    FailedToGetNodePort,
    /// The request is invalid or the arguments of the function are invalid
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
    /// Error occurred in an async thread
    #[error("Error occured in async thread: {0}")]
    JoinErrorInAsyncThread(String),
}
