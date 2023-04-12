// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use thiserror::Error;

/// Specialisation of `std::Result`.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Payment errors.
#[derive(Debug, Error)]
pub enum Error {
    ///
    #[error("Current balance: {balance}. Attempted spend: {attempted_spend}")]
    NotEnoughBalance {
        ///
        balance: sn_dbc::Token,
        ///
        attempted_spend: sn_dbc::Token,
    },
    ///
    #[error("Failed to send tokens due to {0}")]
    CouldNotSendTokens(String),
}
