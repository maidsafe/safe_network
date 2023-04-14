// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use sn_dbc::Error as DbcError;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Specialisation of `std::Result`.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Fee errors.
#[derive(Error, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Error {
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

impl From<DbcError> for Error {
    fn from(error: DbcError) -> Self {
        Error::Dbcs(error.to_string())
    }
}
