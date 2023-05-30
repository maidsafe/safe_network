// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{CreatedDbc, Error, Inputs, Outputs, Result, SpendRequest};

use sn_dbc::{
    rng, Dbc, DbcIdSource, DerivedKey, Hash, InputHistory, PublicAddress, RevealedInput, Token,
    TransactionBuilder,
};

use std::collections::BTreeMap;

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
/// (Disabled for now: DbcReason, can be added later.)
#[allow(unused)]
pub(crate) fn create_transfer(
    available_dbcs: Vec<(Dbc, DerivedKey)>,
    recipients: Vec<(Token, DbcIdSource)>,
    change_to: PublicAddress,
) -> Result<Outputs> {
    // We need to select the necessary number of dbcs from those that we were passed.
    let selected_inputs = select_inputs(available_dbcs, recipients, change_to)?;
    create_transfer_with(selected_inputs)
}

/// Select the necessary number of dbcs from those that we were passed.
fn select_inputs(
    available_dbcs: Vec<(Dbc, DerivedKey)>,
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

    for (dbc, derived_key) in available_dbcs {
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
fn verify_amounts(total_input_amount: Token, total_output_amount: Token) -> Result<()> {
    if total_output_amount > total_input_amount {
        return Err(Error::NotEnoughBalance(total_input_amount.to_string()));
    }
    Ok(())
}

/// The tokens of the input dbcs will be transfered to the
/// new dbcs (and a change dbc if any), which are returned from this function.
/// This does not register the transaction in the network.
/// To do that, the `signed_spends` of each new dbc, has to be uploaded
/// to the network. When those same signed spends can be retrieved from
/// enough peers in the network, the transaction will be completed.
fn create_transfer_with(selected_inputs: Inputs) -> Result<Outputs> {
    let Inputs {
        dbcs_to_spend,
        recipients,
        change: (change, change_to),
        ..
    } = selected_inputs;

    let mut inputs = vec![];
    let mut src_txs = BTreeMap::new();
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
            input_src_tx: dbc.src_tx.clone(),
        };
        inputs.push(input);
        let _ = src_txs.insert(dbc.id(), dbc.src_tx);
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
        .map_err(Box::new)
        .map_err(Error::Dbcs)?;

    let tx_hash = dbc_builder.spent_tx.hash();

    let signed_spends: BTreeMap<_, _> = dbc_builder
        .signed_spends()
        .into_iter()
        .map(|spend| (spend.dbc_id(), spend))
        .collect();

    // We must have a source transaction for each signed spend (i.e. the tx where the dbc was created).
    // These are required to upload the spends to the network.
    if !signed_spends
        .iter()
        .all(|(dbc_id, _)| src_txs.contains_key(*dbc_id))
    {
        return Err(Error::DbcReissueFailed(
            "Not all signed spends could be matched to a source dbc transaction.".to_string(),
        ));
    }

    let mut all_spend_requests = vec![];
    for (dbc_id, signed_spend) in signed_spends.into_iter() {
        let parent_tx = src_txs.get(dbc_id).ok_or(Error::DbcReissueFailed(format!(
            "Missing source dbc tx of {dbc_id:?}!"
        )))?;

        let spend_requests = SpendRequest {
            signed_spend: signed_spend.clone(),
            parent_tx: parent_tx.clone(),
        };

        all_spend_requests.push(spend_requests);
    }

    // Perform validations of input tx and signed spends,
    // as well as building the output DBCs.
    let mut created_dbcs: Vec<_> = dbc_builder
        .build()
        .map_err(Box::new)
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
        tx_hash,
        created_dbcs,
        change_dbc,
        all_spend_requests,
    })
}
