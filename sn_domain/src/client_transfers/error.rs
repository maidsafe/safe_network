// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use sn_dbc::Error as DbcError;

use thiserror::Error;

pub(crate) type Result<T> = std::result::Result<T, Error>;

/// Error type returned by the API
#[derive(Debug, Error)]
#[allow(clippy::large_enum_variant)]
#[non_exhaustive]
pub enum Error {
    /// Not enough balance to perform a transaction
    #[error("Not enough balance: {0}")]
    NotEnoughBalance(String),
    /// An error from the `sn_dbc` crate.
    #[error("Dbc error: {0}")]
    Dbcs(#[from] Box<DbcError>),
    /// DbcReissueFailed
    #[error("DbcReissueFailed: {0}")]
    DbcReissueFailed(String),
}
