// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use sn_dbc::{
    rng, Dbc, DbcTransaction, Error as DbcError, Hash, InputHistory, MainKey, RevealedAmount,
    RevealedInput, Token, TransactionBuilder,
};

use std::{fmt::Debug, result};
use thiserror::Error;

/// Amount of tokens to be owned by the Genesis DBC.
/// At the inception of the Network a total supply of 4,294,967,295 whole tokens will be created.
/// Each whole token can be subdivided 10^9 times,
/// thus creating a total of 4,294,967,295,000,000,000 available units.
pub const GENESIS_DBC_AMOUNT: u64 = u32::MAX as u64 * u64::pow(10, 9);

/// A specialised `Result` type for types crate.
pub type Result<T> = result::Result<T, Error>;

/// Main error type for the crate.
#[derive(Error, Debug, Clone)]
// #[non_exhaustive]
#[non_exhaustive]
pub enum Error {
    /// Error occurred when minting the Genesis DBC.
    #[error("Genesis DBC error:: {0}")]
    GenesisDbcError(String),
    /// FailedToParseReason
    #[error("Failed to parse reason: {0}")]
    FailedToParseReason(#[from] DbcError),
}

/// Generate the genesis DBC.
///
/// Requires the initial section key to sign the share and a different secret key for the DBC.
///
/// The genesis DBC will be created using a different key from the initial section key. This is
/// because the genesis DBC, along with its secret key, will be publicly available for auditing
/// purposes. It needs to be a set rather than just a key because a spent proof share gets
/// generated, which requires a key set. We can't use the same key for the genesis DBC and section
/// because if the genesis DBC is publicly available, the secret key could be used to create a bad
/// section that would be accepted by the network.
#[allow(clippy::result_large_err)]
pub fn create_genesis_dbc(genesis_main_key: &MainKey) -> Result<Dbc> {
    let rng = &mut rng::thread_rng();

    let dbc_id_src = genesis_main_key.random_dbc_id_src(rng);
    let derived_key = genesis_main_key.derive_key(&dbc_id_src.derivation_index);
    let revealed_amount = RevealedAmount::from_amount(GENESIS_DBC_AMOUNT, rng);

    // Use the same key as the input and output of Genesis Tx.
    // The src tx is empty as this is the first DBC.
    let genesis_input = InputHistory {
        input: RevealedInput::new(derived_key, revealed_amount),
        input_src_tx: DbcTransaction {
            inputs: vec![],
            outputs: vec![],
        },
    };

    let dbc_builder = TransactionBuilder::default()
        .add_input(genesis_input)
        .add_output(Token::from_nano(GENESIS_DBC_AMOUNT), dbc_id_src)
        .build(Hash::default(), rng::thread_rng())
        .map_err(|err| {
            Error::GenesisDbcError(format!(
                "Failed to build the DBC transaction for genesis DBC: {err}",
            ))
        })?;

    // build the output DBCs
    let output_dbcs = dbc_builder.build_without_verifying().map_err(|err| {
        Error::GenesisDbcError(format!(
            "DBC builder failed to create output genesis DBC: {err}",
        ))
    })?;

    // just one output DBC is expected which is the genesis DBC
    let (genesis_dbc, _) = output_dbcs.into_iter().next().ok_or_else(|| {
        Error::GenesisDbcError(
            "DBC builder (unexpectedly) contains an empty set of outputs.".to_string(),
        )
    })?;

    Ok(genesis_dbc)
}
