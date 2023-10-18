// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    rng, CashNote, DerivationIndex, DerivedSecretKey, Hash, Input, MainPubkey, NanoTokens,
    SignedSpend, Transaction, TransactionBuilder, Transfer, UniquePubkey,
};
use crate::{Error, Result};

use std::collections::BTreeMap;
use xor_name::XorName;

/// Offline Transfer
/// This struct contains all the necessary information to carry out the transfer.
/// The created cash_notes and change cash_note from a transfer
/// of tokens from one or more cash_notes, into one or more new cash_notes.
#[derive(custom_debug::Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct OfflineTransfer {
    /// This is the transaction where all the below
    /// spends were made and cash_notes created.
    pub tx: Transaction,
    /// The cash_notes that were created containing
    /// the tokens sent to respective recipient.
    #[debug(skip)]
    pub created_cash_notes: Vec<CashNote>,
    /// The cash_note holding surplus tokens after
    /// spending the necessary input cash_notes.
    #[debug(skip)]
    pub change_cash_note: Option<CashNote>,
    /// The parameters necessary to send all spend requests to the network.
    pub all_spend_requests: Vec<SignedSpend>,
}

pub type PaymentDetails = (Transfer, MainPubkey, NanoTokens);

/// Xorname of data from which the content was fetched, mapping to the CashNote UniquePubkey (its id on disk)
/// the main key for that CashNote and the value
pub type ContentPaymentsIdMap = BTreeMap<XorName, Vec<PaymentDetails>>;

/// The input details necessary to
/// carry out a transfer of tokens.
#[derive(Debug)]
struct TranferInputs {
    /// The selected cash_notes to spend, with the necessary amounts contained
    /// to transfer the below specified amount of tokens to each recipients.
    pub cash_notes_to_spend: Vec<(CashNote, DerivedSecretKey)>,
    /// The amounts and cash_note ids for the cash_notes that will be created to hold the transferred tokens.
    pub recipients: Vec<(NanoTokens, MainPubkey, DerivationIndex)>,
    /// Any surplus amount after spending the necessary input cash_notes.
    pub change: (NanoTokens, MainPubkey),
}

/// A function for creating an offline transfer of tokens.
/// This is done by creating new cash_notes to the recipients (and a change cash_note if any)
/// by selecting from the available input cash_notes, and creating the necessary
/// spends to do so.
///
/// Those signed spends are found in each new cash_note, and must be uploaded to the network
/// for the transaction to take effect.
/// The peers will validate each signed spend they receive, before accepting it.
/// Once enough peers have accepted all the spends of the transaction, and serve
/// them upon request, the transaction will be completed.
pub fn create_offline_transfer(
    available_cash_notes: Vec<(CashNote, DerivedSecretKey)>,
    recipients: Vec<(NanoTokens, MainPubkey, DerivationIndex)>,
    change_to: MainPubkey,
    reason_hash: Hash,
) -> Result<OfflineTransfer> {
    let total_output_amount = recipients
        .iter()
        .try_fold(NanoTokens::zero(), |total, (amount, _, _)| {
            total.checked_add(*amount)
        })
        .ok_or_else(|| {
            Error::CashNoteReissueFailed(
                "Overflow occurred while summing the amounts for the recipients.".to_string(),
            )
        })?;

    // We need to select the necessary number of cash_notes from those that we were passed.
    let (cash_notes_to_spend, change_amount) =
        select_inputs(available_cash_notes, total_output_amount)?;

    let selected_inputs = TranferInputs {
        cash_notes_to_spend,
        recipients,
        change: (change_amount, change_to),
    };

    create_offline_transfer_with(selected_inputs, reason_hash)
}

/// Select the necessary number of cash_notes from those that we were passed.
fn select_inputs(
    available_cash_notes: Vec<(CashNote, DerivedSecretKey)>,
    total_output_amount: NanoTokens,
) -> Result<(Vec<(CashNote, DerivedSecretKey)>, NanoTokens)> {
    let mut cash_notes_to_spend = Vec::new();
    let mut total_input_amount = NanoTokens::zero();
    let mut change_amount = total_output_amount;

    for (cash_note, derived_key) in available_cash_notes {
        let input_key = cash_note.unique_pubkey();

        let cash_note_balance = match cash_note.value() {
            Ok(token) => token,
            Err(err) => {
                warn!(
                    "Ignoring input CashNote (id: {input_key:?}) due to missing an output: {err:?}"
                );
                continue;
            }
        };

        // Add this CashNote as input to be spent.
        cash_notes_to_spend.push((cash_note, derived_key));

        // Input amount increases with the amount of the cash_note.
        total_input_amount = total_input_amount.checked_add(cash_note_balance)
            .ok_or_else(|| {
                Error::CashNoteReissueFailed(
                    "Overflow occurred while increasing total input amount while trying to cover the output CashNotes."
                    .to_string(),
            )
            })?;

        // If we've already combined input CashNotes for the total output amount, then stop.
        match change_amount.checked_sub(cash_note_balance) {
            Some(pending_output) => {
                change_amount = pending_output;
                if change_amount.as_nano() == 0 {
                    break;
                }
            }
            None => {
                change_amount =
                    NanoTokens::from(cash_note_balance.as_nano() - change_amount.as_nano());
                break;
            }
        }
    }

    // Make sure total input amount gathered with input CashNotes are enough for the output amount
    if total_output_amount > total_input_amount {
        return Err(Error::NotEnoughBalance(
            total_input_amount,
            total_output_amount,
        ));
    }

    Ok((cash_notes_to_spend, change_amount))
}

/// The tokens of the input cash_notes will be transfered to the
/// new cash_notes (and a change cash_note if any), which are returned from this function.
/// This does not register the transaction in the network.
/// To do that, the `signed_spends` of each new cash_note, has to be uploaded
/// to the network. When those same signed spends can be retrieved from
/// enough peers in the network, the transaction will be completed.
fn create_offline_transfer_with(
    selected_inputs: TranferInputs,
    reason_hash: Hash,
) -> Result<OfflineTransfer> {
    let TranferInputs {
        change: (change, change_to),
        ..
    } = selected_inputs;

    let mut inputs = vec![];
    let mut src_txs = BTreeMap::new();
    for (cash_note, derived_key) in selected_inputs.cash_notes_to_spend {
        let token = match cash_note.value() {
            Ok(token) => token,
            Err(err) => {
                warn!("Ignoring cash_note, as it didn't have the correct derived key: {err}");
                continue;
            }
        };
        let input = Input {
            unique_pubkey: cash_note.unique_pubkey(),
            amount: token,
        };
        inputs.push((input, derived_key, cash_note.src_tx.clone()));
        let _ = src_txs.insert(cash_note.unique_pubkey(), cash_note.src_tx);
    }

    let mut tx_builder = TransactionBuilder::default()
        .add_inputs(inputs)
        .add_outputs(selected_inputs.recipients);

    let mut rng = rng::thread_rng();
    let derivation_index = UniquePubkey::random_derivation_index(&mut rng);
    let change_id = change_to.new_unique_pubkey(&derivation_index);
    if !change.is_zero() {
        tx_builder = tx_builder.add_output(change, change_to, derivation_index);
    }

    // Finalize the tx builder to get the cash_note builder.
    let cash_note_builder = tx_builder.build(reason_hash)?;

    let tx = cash_note_builder.spent_tx.clone();

    let signed_spends: BTreeMap<_, _> = cash_note_builder
        .signed_spends()
        .into_iter()
        .map(|spend| (spend.unique_pubkey(), spend))
        .collect();

    // We must have a source transaction for each signed spend (i.e. the tx where the cash_note was created).
    // These are required to upload the spends to the network.
    if !signed_spends
        .iter()
        .all(|(unique_pubkey, _)| src_txs.contains_key(*unique_pubkey))
    {
        return Err(Error::CashNoteReissueFailed(
            "Not all signed spends could be matched to a source cash_note transaction.".to_string(),
        ));
    }

    let mut all_spend_requests = vec![];
    for (_, signed_spend) in signed_spends.into_iter() {
        all_spend_requests.push(signed_spend.to_owned());
    }

    // Perform validations of input tx and signed spends,
    // as well as building the output CashNotes.
    let mut created_cash_notes: Vec<_> = cash_note_builder
        .build()?
        .into_iter()
        .map(|(cash_note, _)| cash_note)
        .collect();

    let mut change_cash_note = None;
    created_cash_notes.retain(|created| {
        if created.unique_pubkey() == change_id {
            change_cash_note = Some(created.clone());
            false
        } else {
            true
        }
    });

    Ok(OfflineTransfer {
        tx,
        created_cash_notes,
        change_cash_note,
        all_spend_requests,
    })
}
