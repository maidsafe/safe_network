// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::domain::{fees, storage::Error as StorageError};

use sn_dbc::Token;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors related to node handling of transfers.
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
    /// One or more parent spends of a requested spend had a different dst tx hash than the signed spend src tx hash.
    #[error(
        "The signed spend src tx ({signed_src_tx_hash:?}) did not match the provided source tx's hash: {provided_src_tx_hash:?}"
    )]
    TxSourceMismatch {
        /// The signed spend src tx hash.
        signed_src_tx_hash: sn_dbc::Hash,
        /// The hash of the provided source tx.
        provided_src_tx_hash: sn_dbc::Hash,
    },
    /// One or more parent spends of a requested spend had a different dst tx hash than the signed spend src tx hash.
    #[error(
        "The signed spend src tx ({signed_src_tx_hash:?}) did not match a valid parent's dst tx hash: {parent_dst_tx_hash:?}. The trail is invalid."
    )]
    TxTrailMismatch {
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
    /// Storage error.
    #[error("Storage error {0:?}")]
    Storage(#[from] StorageError),
}
