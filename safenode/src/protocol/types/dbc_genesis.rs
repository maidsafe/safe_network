// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use sn_dbc::{
    rng, Dbc, DbcId, DbcIdSource, DbcTransaction, DerivedKey, Error as DbcError, Hash,
    InputHistory, MainKey, PublicAddress, RevealedAmount, RevealedInput, Token, TransactionBuilder,
};

use std::{collections::BTreeSet, fmt::Debug, result};
use thiserror::Error;

/// Amount of tokens to be owned by the Genesis DBC.
/// At the inception of the Network a total supply of 4,294,967,295 whole tokens will be created.
/// Each whole token can be subdivided 10^9 times,
/// thus creating a total of 4,294,967,295,000,000,000 available units.
pub const GENESIS_DBC_AMOUNT: u64 = u32::MAX as u64 * u64::pow(10, 9);

/// A specialised `Result` type for types crate.
pub type GenesisResult<T> = result::Result<T, GenesisError>;

/// Main error type for the crate.
#[derive(Error, Debug, Clone)]
// #[non_exhaustive]
#[non_exhaustive]
pub enum GenesisError {
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
pub fn create_genesis_dbc(genesis_main_key: &MainKey) -> GenesisResult<Dbc> {
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
            GenesisError::GenesisDbcError(format!(
                "Failed to build the DBC transaction for genesis DBC: {err}",
            ))
        })?;

    // build the output DBCs
    let output_dbcs = dbc_builder.build_without_verifying().map_err(|err| {
        GenesisError::GenesisDbcError(format!(
            "DBC builder failed to create output genesis DBC: {err}",
        ))
    })?;

    // just one output DBC is expected which is the genesis DBC
    let (genesis_dbc, _) = output_dbcs.into_iter().next().ok_or_else(|| {
        GenesisError::GenesisDbcError(
            "DBC builder (unexpectedly) contains an empty set of outputs.".to_string(),
        )
    })?;

    Ok(genesis_dbc)
}

pub(super) type Result<T> = std::result::Result<T, Error>;

/// Error type returned by the API
#[derive(Debug, Error)]
#[allow(clippy::large_enum_variant)]
#[non_exhaustive]
pub enum Error {
    /// Not enough balance to perform a transaction
    #[error("Not enough balance: {0}")]
    NotEnoughBalance(String),
    /// Not enough was paid in fees for the Elders to process the spend.
    #[error("Too low amount for the transfer. Highest required fee: {0:?}.")]
    FeeTooLow(Token),
    /// DbcError
    #[error("DbcError: {0}")]
    DbcError(#[from] DbcError),
    /// DbcReissueError
    #[error("DbcReissueError: {0}")]
    DbcReissueError(String),
    /// Verification of DBC validly signed by a known section failed
    #[error("DBC validity verification failed: {0}")]
    DbcVerificationFailed(String),
}

/// Send the tokens to the specified destination keys, using the provided dbcs.
/// The new dbcs that are created, one per specified destination, will have the
/// unique id which is the public key of the `OwnerOnce` instances provided.
///
/// Transfer fees will be paid if not in data-network.
/// The input dbcs will be spent on the network, and the resulting
/// dbcs (and change dbc if any) are returned.
/// NB: We are skipping the DbcReason arg for now. It can be added later.
#[allow(clippy::result_large_err)]
pub fn send_tokens(
    dbcs: Vec<(Dbc, DerivedKey)>,
    recipients: Vec<(Token, DbcIdSource)>,
    change_to: PublicAddress,
) -> Result<ReissueOutputs> {
    // We need to select the necessary number of dbcs from those that we were passed.
    // This will also account for any fees.
    let reissue_inputs = select_inputs(dbcs, recipients, change_to)?;
    reissue(reissue_inputs)
}

/// The input dbcs will be spent on the network, and the resulting
/// dbcs (and change dbc if any) are returned.
/// This will pay transfer fees if not in data-network.
#[allow(clippy::result_large_err)]
fn reissue(
    // client: &Client,
    reissue_inputs: ReissueInputs,
    // reason: DbcReason,
) -> Result<ReissueOutputs> {
    let ReissueInputs {
        input_dbcs,
        outputs,
        change: (change, change_to),
    } = reissue_inputs;

    let mut inputs = vec![];
    for (dbc, derived_key) in input_dbcs {
        let revealed_amount = match dbc.revealed_amount(&derived_key) {
            Ok(amount) => amount,
            Err(err) => {
                warn!("Ignoring dbc, as it didn't have the correct derived key: {err}");
                continue;
            }
        };
        let input = InputHistory {
            input: RevealedInput::new(derived_key, revealed_amount),
            input_src_tx: dbc.src_tx,
        };
        inputs.push(input);
    }

    let mut tx_builder = TransactionBuilder::default()
        .add_inputs(inputs)
        .add_outputs(outputs);

    let mut rng = rng::thread_rng();

    let dbc_id_src = change_to.random_dbc_id_src(&mut rng);
    if change.as_nano() > 0 {
        tx_builder = tx_builder.add_output(change, dbc_id_src);
    }

    // Finalize the tx builder to get the dbc builder.
    let dbc_builder = tx_builder
        .build(Hash::default(), &mut rng)
        .map_err(Error::DbcError)?;

    let inputs = dbc_builder.signed_spends();
    let spent_dbcs = inputs.iter().map(|spent| spent.dbc_id()).cloned().collect();

    // Perform verifications of input TX and spentproofs,
    // as well as building the output DBCs.
    let mut output_dbcs = dbc_builder.build().map_err(Error::DbcError)?;

    let mut change_dbc = None;
    output_dbcs.retain(|(dbc, _)| {
        if dbc.public_address() == &change_to && change.as_nano() > 0 {
            change_dbc = Some(dbc.clone());
            false
        } else {
            true
        }
    });

    Ok(ReissueOutputs {
        outputs: output_dbcs,
        change: change_dbc,
        spent_dbcs,
    })
}

///
#[allow(clippy::result_large_err)]
pub(crate) fn select_inputs(
    inputs: Vec<(Dbc, DerivedKey)>,
    outputs: Vec<(Token, DbcIdSource)>,
    change_to: PublicAddress,
) -> Result<ReissueInputs> {
    // We'll combine one or more input DBCs and reissue:
    // - one output DBC per recipient,
    // - and a single DBC for the change - if any - which will be returned from this function.
    let mut input_dbcs = Vec::new();
    let mut total_input_amount = Token::zero();
    let total_output_amount = outputs
        .iter()
        .fold(Some(Token::zero()), |total, (amount, _)| {
            total.and_then(|t| t.checked_add(*amount))
        })
        .ok_or_else(|| {
            Error::DbcReissueError(
                "Overflow occurred while summing the output amounts for the output DBCs."
                    .to_string(),
            )
        })?;

    let mut change_amount = total_output_amount;

    for (dbc, derived_key) in inputs {
        let input_key = dbc.id();

        let dbc_balance = match dbc.revealed_amount(&derived_key) {
            Ok(revealed_amount) => Token::from_nano(revealed_amount.value()),
            Err(err) => {
                warn!("Ignoring input Dbc (id: {input_key:?}) due to not having correct derived key: {err:?}");
                continue;
            }
        };

        // Add this Dbc as input to be spent.
        input_dbcs.push((dbc, derived_key));

        // Input amount increases with the amount of the dbc.
        total_input_amount = total_input_amount.checked_add(dbc_balance)
            .ok_or_else(|| {
                Error::DbcReissueError(
                    "Overflow occurred while increasing total input amount while trying to cover the output DBCs."
                    .to_string(),
            )
            })?;

        // If we've already combined input DBCs for the total output amount, then stop.
        match change_amount.checked_sub(dbc_balance) {
            Some(pending_output) => {
                change_amount = pending_output;
                if change_amount.as_nano() == 0 {
                    break;
                }
            }
            None => {
                change_amount = Token::from_nano(dbc_balance.as_nano() - change_amount.as_nano());
                break;
            }
        }
    }

    // If not enough spendable was found, this check will return an error.
    verify_amounts(total_input_amount, total_output_amount)?;

    Ok(ReissueInputs {
        input_dbcs,
        outputs,
        change: (change_amount, change_to),
    })
}

// Make sure total input amount gathered with input DBCs are enough for the output amount
#[allow(clippy::result_large_err)]
fn verify_amounts(total_input_amount: Token, total_output_amount: Token) -> Result<()> {
    if total_output_amount > total_input_amount {
        return Err(Error::NotEnoughBalance(total_input_amount.to_string()));
    }
    Ok(())
}

///
#[derive(Debug)]
pub struct ReissueInputs {
    /// The dbcs to spend, as to send tokens.
    pub input_dbcs: Vec<(Dbc, DerivedKey)>,
    /// The dbcs that will be created, holding the tokens to send.
    pub outputs: Vec<(Token, DbcIdSource)>,
    /// Any surplus amount after spending the necessary input dbcs.
    pub change: (Token, PublicAddress),
}

/// The results of reissuing dbcs.
#[derive(Debug)]
pub struct ReissueOutputs {
    /// The dbcs holding the tokens that were sent.
    pub outputs: Vec<(Dbc, RevealedAmount)>,
    /// The dbc holding surplus amount after spending the necessary input dbcs.
    pub change: Option<Dbc>,
    /// The dbcs we spent when reissuing.
    pub spent_dbcs: BTreeSet<DbcId>,
}
