// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod error;

use error::{Error, Result};

use sn_dbc::{
    rng, Dbc, DbcId, DbcIdSource, DerivedKey, Hash, InputHistory, PublicAddress, RevealedAmount,
    RevealedInput, Token, TransactionBuilder,
};

use std::{collections::BTreeSet, fmt::Debug};

/// The input details to a transfer of tokens.
#[derive(Debug)]
pub struct Inputs {
    /// The dbcs to spend, as to transfer their tokens.
    pub input_dbcs: Vec<(Dbc, DerivedKey)>,
    /// The dbcs that will be created, holding the transferred tokens.
    pub outputs: Vec<(Token, DbcIdSource)>,
    /// Any surplus amount after spending the necessary input dbcs.
    pub change: (Token, PublicAddress),
}

/// The token transfer results.
#[derive(Debug)]
pub struct Outputs {
    /// The dbcs holding the tokens that were transferred.
    pub outputs: Vec<(Dbc, RevealedAmount)>,
    /// The ids of the dbcs we spent in this transfer.
    pub spent_dbcs: BTreeSet<DbcId>,
    /// The dbc holding surplus amount after spending the necessary input dbcs.
    pub change: Option<Dbc>,
}

/// A function for creating an offline transfer of tokens.
/// This is done by creating new dbcs to the recipients (and a change dbc if any)
/// by selecting from the available input dbcs, and creating the necessary
/// spends to do so.
///
/// Those signed spends are found in each new dbc, and must be uploaded to the network
/// for the transactions to take effect.
/// The peers will validate each signed spend they receive, before accepting it.
/// Once enough peers have accepted all the spends of the transaction, and serve
/// them upon request, the transaction will be completed.
///
/// (Disabled for now: Transfer fees will be added if not in data-network.)
/// (Disabled for now: DbcReason, can be added later.)
#[allow(clippy::result_large_err)]
pub fn create_transfer(
    dbcs: Vec<(Dbc, DerivedKey)>,
    recipients: Vec<(Token, DbcIdSource)>,
    change_to: PublicAddress,
) -> Result<Outputs> {
    // We need to select the necessary number of dbcs from those that we were passed.
    // This will also account for any fees.
    let send_inputs = select_inputs(dbcs, recipients, change_to)?;
    transfer(send_inputs)
}

/// The tokens of the input dbcs will be transfered to the
/// new dbcs (and a change dbc if any), which are returned from this function.
/// This does not register the transaction in the network.
/// To do that, the `signed_spends` of each new dbc, has to be uploaded
/// to the network. When those same signed spends can be retrieved from
/// enough peers in the network, the transaction will be completed.
#[allow(clippy::result_large_err)]
fn transfer(send_inputs: Inputs) -> Result<Outputs> {
    let Inputs {
        input_dbcs,
        outputs,
        change: (change, change_to),
    } = send_inputs;

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
        .map_err(Error::Dbcs)?;

    let signed_spends = dbc_builder.signed_spends();
    let spent_dbcs = signed_spends.iter().map(|spent| *spent.dbc_id()).collect();

    // Perform verifications of input TX and spentproofs,
    // as well as building the output DBCs.
    let mut output_dbcs = dbc_builder.build().map_err(Error::Dbcs)?;

    let mut change_dbc = None;
    output_dbcs.retain(|(dbc, _)| {
        if dbc.public_address() == &change_to && change.as_nano() > 0 {
            change_dbc = Some(dbc.clone());
            false
        } else {
            true
        }
    });

    Ok(Outputs {
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
) -> Result<Inputs> {
    let mut input_dbcs = Vec::new();
    let mut total_input_amount = Token::zero();
    let total_output_amount = outputs
        .iter()
        .fold(Some(Token::zero()), |total, (amount, _)| {
            total.and_then(|t| t.checked_add(*amount))
        })
        .ok_or_else(|| {
            Error::DbcReissueFailed(
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
                Error::DbcReissueFailed(
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

    Ok(Inputs {
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
