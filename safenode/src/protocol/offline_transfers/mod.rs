// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

//! This module contains the functions for creating an offline transfer of tokens.
//! This is done by emptying the input dbcs, thereby rendering them spent, and creating
//! new dbcs to the recipients (and a change dbc if any) containing the transferred tokens.
//! When a transfer is created, it is not yet registered on the network. The signed spends of
//! the transfer is found in the new dbcs, and must be uploaded to the network to take effect.
//! The peers will validate each signed spend they receive, before accepting it.
//! Once enough peers have accepted all the spends of the transaction, and serve them upon request,
//! the transfer is completed and globally recognised.
//!
//! The transfer is created by selecting from the available input dbcs, and creating the necessary
//! spends to do so. The input dbcs are selected by the user, and the spends are created by this
//! module. The user can select the input dbcs by specifying the amount of tokens they want to
//! transfer, and the module will select the necessary dbcs to transfer that amount. The user can
//! also specify the amount of tokens they want to transfer to each recipient, and the module will
//! select the necessary dbcs to transfer that amount to each recipient.
//!
//! On the difference between a transfer and a transaction.
//! The difference is subtle, but very much there. A transfer is a higher level concept, it is the
//! sending of tokens from one address to another. Or many.
//! A dbc transaction is the lower layer concept where the blinded inputs and outputs are specified.

mod error;

use error::{Error, Result};

use sn_dbc::{
    rng, Dbc, DbcIdSource, DerivedKey, Hash, InputHistory, PublicAddress, RevealedAmount,
    RevealedInput, Token, TransactionBuilder,
};

use std::fmt::Debug;

/// The input details necessary to
/// carry out a transfer of tokens.
#[derive(Debug)]
pub struct Inputs {
    /// The selected dbcs to spend, with the necessary amounts contained
    /// to transfer the below specified amount of tokens to each recipients.
    pub dbcs_to_spend: Vec<(Dbc, DerivedKey)>,
    /// The amounts and dbc ids for the dbcs that will be created to hold the transferred tokens.
    pub recipients: Vec<(Token, DbcIdSource)>,
    /// Any surplus amount after spending the necessary input dbcs.
    pub change: (Token, PublicAddress),
}

/// The created dbcs and change dbc from a transfer
/// of tokens from one or more dbcs, into one or more new dbcs.
#[derive(Debug)]
pub struct Outputs {
    /// The dbcs that were created containing
    /// the tokens sent to respective recipient.
    pub created_dbcs: Vec<CreatedDbc>,
    /// The dbc holding surplus tokens after
    /// spending the necessary input dbcs.
    pub change_dbc: Option<Dbc>,
}

/// A resulting dbc from a token transfer.
#[derive(Debug, Clone)]
pub struct CreatedDbc {
    /// The dbc that was created.
    pub dbc: Dbc,
    /// This is useful for the sender to know how much they sent to each recipient.
    /// They can't know this from the dbc itself, as the amount is encrypted.
    pub amount: RevealedAmount,
}

/// A function for creating an offline transfer of tokens.
/// This is done by creating new dbcs to the recipients (and a change dbc if any)
/// by selecting from the available input dbcs, and creating the necessary
/// spends to do so.
///
/// Those signed spends are found in each new dbc, and must be uploaded to the network
/// for the transaction to take effect.
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
        dbcs_to_spend,
        recipients,
        change: (change, change_to),
    } = send_inputs;

    let mut inputs = vec![];
    for (dbc, derived_key) in dbcs_to_spend {
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
        .add_outputs(recipients);

    let mut rng = rng::thread_rng();

    let dbc_id_src = change_to.random_dbc_id_src(&mut rng);
    let change_id = dbc_id_src.dbc_id();
    if change.as_nano() > 0 {
        tx_builder = tx_builder.add_output(change, dbc_id_src);
    }

    // Finalize the tx builder to get the dbc builder.
    let dbc_builder = tx_builder
        .build(Hash::default(), &mut rng)
        .map_err(Error::Dbcs)?;

    // Perform validations of input tx and signed spends,
    // as well as building the output DBCs.
    let mut created_dbcs: Vec<_> = dbc_builder
        .build()
        .map_err(Error::Dbcs)?
        .into_iter()
        .map(|(dbc, amount)| CreatedDbc { dbc, amount })
        .collect();

    let mut change_dbc = None;
    created_dbcs.retain(|created| {
        if created.dbc.id() == change_id {
            change_dbc = Some(created.dbc.clone());
            false
        } else {
            true
        }
    });

    Ok(Outputs {
        created_dbcs,
        change_dbc,
    })
}

/// Select the necessary number of dbcs from those that we were passed.
#[allow(clippy::result_large_err)]
pub(crate) fn select_inputs(
    inputs: Vec<(Dbc, DerivedKey)>,
    recipients: Vec<(Token, DbcIdSource)>,
    change_to: PublicAddress,
) -> Result<Inputs> {
    let mut dbcs_to_spend = Vec::new();
    let mut total_input_amount = Token::zero();
    let total_output_amount = recipients
        .iter()
        .fold(Some(Token::zero()), |total, (amount, _)| {
            total.and_then(|t| t.checked_add(*amount))
        })
        .ok_or_else(|| {
            Error::DbcReissueFailed(
                "Overflow occurred while summing the amounts for the recipients.".to_string(),
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
        dbcs_to_spend.push((dbc, derived_key));

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
        dbcs_to_spend,
        recipients,
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
