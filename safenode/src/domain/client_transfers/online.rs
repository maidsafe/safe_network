// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{CreatedDbc, Error, Inputs, Outputs, Result, SpendRequest};

use crate::{
    client::Client,
    domain::fees::{FeeCiphers, RequiredFee, SpendPriority},
    network::close_group_majority,
    node::NodeId,
    protocol::messages::{Query, QueryResponse, Request, Response, SpendQuery},
};

use sn_dbc::{
    rng, Dbc, DbcId, DbcIdSource, DerivedKey, Hash, InputHistory, PublicAddress, RevealedAmount,
    RevealedInput, Token, TransactionBuilder,
};

use std::collections::{BTreeMap, BTreeSet};

/// A function for creating an online transfer of tokens.
/// This is done by creating new dbcs to the recipients (and a change dbc if any)
/// by selecting from the available input dbcs, and creating the necessary
/// spends to do so. Additionally, fees for each of those inputs will be queried for
/// and the nodes to pay the fee to will be included in the outputs.
///
/// The resulting signed spends are found in each new dbc, and must be uploaded to
/// the network for the transaction to take effect.
/// The nodes will validate each signed spend they receive, before accepting it.
/// Once enough nodes have accepted all the spends of the transaction, and serve
/// them upon request, the transaction will be completed.
///
/// (Disabled for now: DbcReason, can be added later.)
pub(crate) async fn create_transfer(
    available_dbcs: Vec<(Dbc, DerivedKey)>,
    recipients: Vec<(Token, DbcIdSource)>,
    change_to: PublicAddress,
    client: &Client,
) -> Result<Outputs> {
    // We need to select the necessary number of dbcs from those that we were passed.
    // This will also account for any fees.
    let selected_inputs = select_inputs(available_dbcs, recipients, change_to, client).await?;
    create_transfer_with(selected_inputs)
}

/// Select the necessary number of dbcs from those that we were passed.
async fn select_inputs(
    available_dbcs: Vec<(Dbc, DerivedKey)>,
    mut recipients: Vec<(Token, DbcIdSource)>,
    change_to: PublicAddress,
    client: &Client,
) -> Result<Inputs> {
    // We'll combine one or more input dbcs and reissue:
    // - one output dbc per recipient,
    // - one output dbc for each node that will be paid a fee,
    // - and a single dbc for the change - if any - which will be returned from this function.
    let mut dbcs_to_spend = vec![];
    let mut total_input_amount = Token::zero();
    let mut total_output_amount = recipients
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
    let mut node_fees_per_input = BTreeMap::new();
    let mut fees_paid = Token::zero();

    for (dbc, derived_key) in available_dbcs {
        let dbc_id = dbc.id();

        let dbc_balance = match dbc.revealed_amount(&derived_key) {
            Ok(revealed_amount) => Token::from_nano(revealed_amount.value()),
            Err(err) => {
                warn!("Ignoring input dbc (id: {dbc_id:?}) due to not having correct derived key: {err:?}");
                continue;
            }
        };

        // ------------ fee part start ----------------
        #[cfg(not(feature = "data-network"))]
        let fee_per_input = {
            // Each section will have CLOSE_GROUP_SIZE instances to pay individually.
            let node_fees = match get_fees(dbc_id, client).await {
                Ok(fees) => fees,
                Err(error) => {
                    error!("Could not get fees for input dbc: {dbc_id:?}: {error}");
                    continue;
                }
            };
            let num_responses = node_fees.len();
            let required_responses = close_group_majority();
            if required_responses > num_responses {
                warn!("Not enough nodes contacted for the section to spend the input. Got: {num_responses}, needed: {required_responses}");
                continue;
            }

            // Fees that were not encrypted to us.
            let mut invalid_fees = BTreeSet::new();
            // As the nodes encrypt the amount to the dbc id to be spent, we need to decrypt it.
            let mut decrypted_node_fees = vec![];

            for (node_id, fee) in node_fees {
                match fee.content.decrypt_amount(&derived_key) {
                    Ok(amount) => decrypted_node_fees.push(((node_id, fee), amount)),
                    Err(error) => {
                        error!("Decrypting the fee content from {node_id:?} failed! {error}");
                        let _ = invalid_fees.insert(fee.content.reward_address);
                    }
                }
            }

            let max_invalid_fees = num_responses - required_responses;
            if invalid_fees.len() > max_invalid_fees {
                let valid_responses = num_responses - invalid_fees.len();
                warn!("Not enough valid fees received from nodes to spend the input. Found: {valid_responses}, needed: {required_responses}", );
                continue;
            }

            // Total fee paid to all recipients in the section for this input.
            let fee_per_input = decrypted_node_fees
                .iter()
                .fold(Some(Token::zero()), |total, (_, fee)| {
                    total.and_then(|t| t.checked_add(*fee))
                })
                .ok_or_else(|| Error::DbcReissueFailed(
                    "Overflow occurred while summing the individual node's fees in order to calculate the total amount for the output DBCs."
                        .to_string(),
                ))?;

            let mut node_fees = BTreeMap::new();

            // Add nodes to outputs and generate their fee ciphers.
            decrypted_node_fees
                .iter()
                .for_each(|((node_id, required_fee), fee)| {
                    let dbc_id_src = required_fee
                        .content
                        .reward_address
                        .random_dbc_id_src(&mut rand::thread_rng());
                    recipients.push((*fee, dbc_id_src));
                    let _ = node_fees.insert(node_id.clone(), (required_fee.clone(), dbc_id_src));
                });

            let _ = node_fees_per_input.insert(dbc_id, node_fees);

            fees_paid = fees_paid.checked_add(fee_per_input).ok_or_else(|| {
                Error::DbcReissueFailed(
                    "Overflow occurred while summing all the input fees.".to_string(),
                )
            })?;

            fee_per_input
        };
        // ---------------- fee part end ----------------

        // Add this dbc as input to be spent.
        dbcs_to_spend.push((dbc, derived_key));

        // Input amount increases with the amount of the dbc.
        total_input_amount = total_input_amount.checked_add(dbc_balance)
            .ok_or_else(|| {
                Error::DbcReissueFailed(
                    "Overflow occurred while increasing total input amount while trying to cover the output DBCs."
                    .to_string(),
            )
            })?;

        // ---------------- fee part start ----------------
        {
            // Output amount now increases a bit, as we have to cover the fee as well..
            total_output_amount = total_output_amount.checked_add(fee_per_input)
            .ok_or_else(|| {
                Error::DbcReissueFailed(
                "Overflow occurred while adding node fee in order to calculate the total amount for the output DBCs."
                    .to_string(),
            )
            })?;
            // ..and so does `change_amount` (that we subtract from to know if we've covered `total_output_amount`).
            change_amount = change_amount.checked_add(fee_per_input)
            .ok_or_else(|| {
                Error::DbcReissueFailed(
                "Overflow occurred while adding node fee in order to calculate the total amount for the output DBCs."
                    .to_string(),
            )
            })?;
        }
        // ---------------- fee part end ----------------

        // If we've already combined input dbcs for the total output amount, then stop.
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
        node_fees_per_input,
    })
}

// Make sure total input amount gathered with input dbcs are enough for the output amount
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
        node_fees_per_input,
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
        .map_err(Error::Dbcs)?;

    let tx_hash = dbc_builder.dst_tx.hash();

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

    let outputs = dbc_builder
        .revealed_outputs
        .iter()
        .map(|output| (output.dbc_id, output.revealed_amount))
        .collect();

    let mut all_spend_requests = vec![];
    for (dbc_id, signed_spend) in signed_spends.into_iter() {
        let parent_tx = src_txs.get(dbc_id).ok_or(Error::DbcReissueFailed(format!(
            "Missing source dbc tx of {dbc_id:?}!"
        )))?;

        let node_fees = node_fees_per_input
            .get(dbc_id)
            .ok_or(Error::DbcReissueFailed(format!(
                "Missing source dbc tx of {dbc_id:?}!"
            )))?;

        let spend_requests = SpendRequest {
            signed_spend: signed_spend.clone(),
            parent_tx: parent_tx.clone(),
            fee_ciphers: fee_ciphers(&outputs, node_fees)?,
        };

        all_spend_requests.push(spend_requests);
    }

    // Perform validations of input tx and signed spends,
    // as well as building the output dbcs.
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
        tx_hash,
        created_dbcs,
        change_dbc,
        all_spend_requests,
    })
}

async fn get_fees(dbc_id: DbcId, client: &Client) -> Result<BTreeMap<NodeId, RequiredFee>> {
    let request = Request::Query(Query::Spend(SpendQuery::GetFees {
        dbc_id,
        priority: SpendPriority::Normal,
    }));
    let responses = client
        .send_to_closest(request)
        .await
        .map_err(|e| Error::CouldNotGetFees(e.to_string()))?;

    // We just want to receive at least a majority of results, we don't care about any errors
    // so we log them, but return whatever results we get. If not enough for upper layer, it will error there.
    let results: BTreeMap<_, _> = responses
        .into_iter()
        .flat_map(|res| {
            if let Err(error) = &res {
                warn!("Error when querying for fees: {error}");
            }
            res
        })
        .filter_map(|resp| match resp {
            Response::Query(resp) => Some(resp),
            other => {
                warn!("Unexpected response to fee query: {other:?}");
                None
            }
        })
        .filter_map(|resp| match resp {
            QueryResponse::GetFees(Ok((node_id, fee))) => Some((node_id, fee)),
            QueryResponse::GetFees(Err(error)) => {
                warn!("Fee query unexpectedly failed: {error}");
                None
            }
            other => {
                warn!("Unexpected response to fee query: {other:?}");
                None
            }
        })
        .collect();

    Ok(results)
}

/// This will encrypt the necessary components of a fee payment,
/// so that a node can find and verify the fee paid to them, from
/// within a request to spend a dbc.
///
/// Within all the outputs, will be each fee payment to respective node.
/// We will encrypt the derivation index to each node public addresss, and
/// the amount to the derived key corresponding to the dbc id of each node fee payment.
fn fee_ciphers(
    outputs: &BTreeMap<DbcId, RevealedAmount>,
    node_fees: &BTreeMap<NodeId, (RequiredFee, DbcIdSource)>,
) -> Result<BTreeMap<NodeId, FeeCiphers>> {
    let mut input_fee_ciphers = BTreeMap::new();
    for (node_id, (required_fee, dbc_id_src)) in node_fees {
        // Encrypt the index to the reward address (`PublicAddress`) of the node.
        let derivation_index_cipher = required_fee
            .content
            .reward_address
            .encrypt(&dbc_id_src.derivation_index);

        let dbc_id = dbc_id_src.dbc_id();
        let revealed_amount = outputs
            .get(&dbc_id)
            .ok_or(Error::DbcReissueFailed("Missing output!".to_string()))?;

        // Encrypt the amount to the _derived key_ (i.e. new dbc id).
        let amount_cipher = revealed_amount.encrypt(&dbc_id);
        let _ = input_fee_ciphers.insert(
            node_id.clone(),
            FeeCiphers::new(amount_cipher, derivation_index_cipher),
        );
    }
    Ok(input_fee_ciphers)
}
