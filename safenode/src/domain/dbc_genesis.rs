// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use sn_dbc::{
    rng, Dbc, DbcTransaction, Error as DbcError, Hash, InputHistory, MainKey, RevealedAmount,
    RevealedInput, Token, TransactionBuilder, DbcIdSource,
};

use std::fmt::Debug;
use thiserror::Error;

/// Total supply of tokens that will eventually exist in the network: 4,294,967,295 * 10^9 = 4,294,967,295,000,000,000.
const TOTAL_SUPPLY: u64 = u32::MAX as u64 * u64::pow(10, 9);

/// The secret key for the genesis DBC.
///
/// This key is public for auditing purposes. Hard coding its value means all nodes will be able to
/// validate it.
pub const GENESIS_DBC_SK: &str = "0c5152498fc5b2f9ed691ef875f2c16f1f950910391f7ba1df63e9f0ce4b2780";

/// Number of tokens in the Genesis DBC.
/// At the inception of the Network 30 % of total supply - i.e. 1,288,490,189 - whole tokens will be created.
/// Each whole token can be subdivided 10^9 times,
/// thus creating a total of 1,288,490,189,000,000,000 available units.
pub(super) const GENESIS_DBC_AMOUNT: u64 = (0.3 * TOTAL_SUPPLY as f64) as u64;

/// A specialised `Result` type for dbc_genesis crate.
pub(super) type GenesisResult<T> = Result<T, Error>;

/// Main error type for the crate.
#[derive(Error, Debug, Clone)]
pub enum Error {
    /// Error occurred when creating the Genesis DBC.
    #[error("Genesis DBC error:: {0}")]
    GenesisDbcError(String),
    /// The dbc error reason that parsing failed.
    #[error("Failed to parse reason: {0}")]
    FailedToParseReason(#[from] DbcError),
}

/// Create the genesis DBC.
/// The genesis DBC is the first DBC in the network. It is created without
/// a source transaction, as there was nothing before it.
#[allow(clippy::result_large_err)]
pub fn create_genesis() -> GenesisResult<Dbc> {
    let secret_key = bls::SecretKey::from_hex(GENESIS_DBC_SK).map_err(sn_dbc::Error::Blsttc)?;
    let main_key = MainKey::new(secret_key);
    create_first_dbc_from_key(&main_key)
}

/// Create a first DBC given any key (i.e. not specifically the hard coded genesis key).
/// The derivation index and blinding factor are hard coded to ensure deterministic creation.
/// This is useful in tests.
#[allow(clippy::result_large_err)]
pub(crate) fn create_first_dbc_from_key(first_dbc_key: &MainKey) -> GenesisResult<Dbc> {
    let dbc_id_src = DbcIdSource {
        public_address: first_dbc_key.public_address(),
        derivation_index: [0u8; 32],
    };
    let derived_key = first_dbc_key.derive_key(&dbc_id_src.derivation_index);
    let revealed_amount = RevealedAmount {
        value: GENESIS_DBC_AMOUNT,
        blinding_factor: sn_dbc::BlindingFactor::from_bits([0u8; 32]),
    };

    // Use the same key as the input and output of Genesis Tx.
    // The src tx is empty as this is the first DBC.
    let genesis_input = InputHistory {
        input: RevealedInput::new(derived_key, revealed_amount),
        input_src_tx: DbcTransaction {
            inputs: vec![],
            outputs: vec![],
        },
    };

    let reason = Hash::hash(b"GENESIS");

    let dbc_builder = TransactionBuilder::default()
        .add_input(genesis_input)
        .add_output(Token::from_nano(GENESIS_DBC_AMOUNT), dbc_id_src)
        .build(reason, rng::thread_rng())
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

/// Split a dbc into multiple. ONLY FOR TEST.
#[cfg(test)]
#[allow(clippy::result_large_err, unused)]
pub(super) fn split(
    dbc: &Dbc,
    main_key: &MainKey,
    number: usize,
) -> GenesisResult<Vec<(Dbc, RevealedAmount)>> {
    let rng = &mut rng::thread_rng();

    let derived_key = dbc.derived_key(main_key)?;
    let revealed_amount = dbc.revealed_amount(&derived_key)?;
    let input = InputHistory {
        input: RevealedInput::new(derived_key, revealed_amount),
        input_src_tx: dbc.src_tx.clone(),
    };

    let recipients: Vec<_> = (0..number)
        .map(|_| {
            let dbc_id_src = main_key.random_dbc_id_src(rng);
            let amount = revealed_amount.value() / number as u64;
            (Token::from_nano(amount), dbc_id_src)
        })
        .collect();

    let dbc_builder = TransactionBuilder::default()
        .add_input(input)
        .add_outputs(recipients)
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

    Ok(output_dbcs)
}
