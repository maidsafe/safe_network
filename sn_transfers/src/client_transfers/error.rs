// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{Error as CashNoteError, Nano};

use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

/// Error type returned by the API
#[derive(Debug, Error)]
#[allow(clippy::large_enum_variant)]
#[non_exhaustive]
pub enum Error {
    /// Not enough balance to perform a transaction
    #[error("Not enough balance, {0} available, {1} required")]
    NotEnoughBalance(Nano, Nano),
    /// An error from the `sn_transfers` crate.
    #[error("CashNote error: {0}")]
    CashNotes(#[from] Box<CashNoteError>),
    /// CashNoteReissueFailed
    #[error("CashNoteReissueFailed: {0}")]
    CashNoteReissueFailed(String),
}
