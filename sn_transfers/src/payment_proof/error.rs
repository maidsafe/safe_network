// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use thiserror::Error;
use xor_name::XorName;

pub(crate) type Result<T> = std::result::Result<T, Error>;

/// Error type returned by the payment_proof utilities
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// Failed to build the payment proof tree
    #[error("Failed to build the payment proof tree: {0}")]
    ProofTree(String),
    /// Failed to generate the audit trail for an item
    #[error("Failed to generate the audit trail for leaf item #{index}: {reason}")]
    GenAuditTrail { index: usize, reason: String },
    /// Failed to build an audit trail from the provided information
    #[error("Failed to build an audit trail from the provided information: {0}")]
    InvalidAuditTrail(String),
    /// The given audit trail deemed invalid
    #[error("The given payment proof audit trail failed to self-validate: {0}")]
    AuditTrailSelfValidation(String),
    /// The leaf data for which proof was generated doesn't match the given item
    #[error("The leaf data for which proof was generated doesn't match the given item: {0:?}")]
    AuditTrailItemMismatch(XorName),
    /// The root data of the proof doesn't match the given reason hash
    #[error("The root data of the proof doesn't match the given reason hash: {0}")]
    ReasonHashMismatch(String),
}
