// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::protocol::types::{address::DbcAddress, fees};

use sn_dbc::{Error as DbcError, SignedSpend, Token};

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

/// The type returned by the `sn_routing` message handling methods.
pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;

/// Transfer errors.
#[derive(Error, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Error {
    #[error("The transfer fee is missing.")]
    MissingFee,
    #[error("Invalid fee blinded amount.")]
    InvalidFeeBlindedAmount,
    #[error("Too low amount for the transfer fee: {paid}. Min required: {required}.")]
    FeeTooLow { paid: Token, required: Token },
    #[error(transparent)]
    Fees(#[from] fees::Error),
    #[error("Contacting close group of parent spends failed: {0}.")]
    SpendParentCloseGroupIssue(String),
    /// An error from the `sn_dbc` crate.
    #[error("Dbc error: {0}")]
    Dbcs(String),
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
    /// Not enough space to store the value.
    #[error("Not enough space")]
    NotEnoughSpace,
}

impl From<DbcError> for Error {
    fn from(error: DbcError) -> Self {
        Error::Dbcs(error.to_string())
    }
}
