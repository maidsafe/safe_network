// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::protocol::messages::NodeId;

use super::StorageError;

// FIMXE: these should be defined within the protocol rather than pulled another crate.
use sn_dbc::{DbcId, Error as DbcError, Hash, SignedSpend, Token};

use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fmt::Debug};
use thiserror::Error;

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
