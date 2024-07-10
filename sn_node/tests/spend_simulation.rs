// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod common;
use assert_fs::TempDir;
use common::client::{get_client_and_funded_wallet, get_wallet};
use eyre::{bail, OptionExt, Report, Result};
use itertools::Itertools;
use rand::{seq::IteratorRandom, Rng};
use sn_client::Client;
use sn_logging::LogBuilder;
use sn_networking::{GetRecordError, NetworkError};
use sn_transfers::{
    rng, CashNote, DerivationIndex, HotWallet, MainPubkey, NanoTokens, OfflineTransfer,
    SpendAddress, SpendReason, Transaction, UniquePubkey,
};
use std::{
    collections::{btree_map::Entry, BTreeMap, BTreeSet},
    fmt::Display,
    path::PathBuf,
    time::Duration,
};
use tokio::sync::mpsc;
use tracing::*;

const MAX_WALLETS: usize = 15;
const MAX_CYCLES: usize = 10;
const AMOUNT_PER_RECIPIENT: NanoTokens = NanoTokens::from(1000);
/// The chance for an double spend to happen. 1 in X chance.
const ONE_IN_X_CHANCE_FOR_AN_ATTACK: u32 = 3;

enum WalletAction {
    Send {
        recipients: Vec<(NanoTokens, MainPubkey, DerivationIndex)>,
    },
    DoubleSpend {
        input_cashnotes_to_double_spend: Vec<CashNote>,
        to: (NanoTokens, MainPubkey, DerivationIndex),
    },
    ReceiveCashNotes {
        from: WalletId,
        cashnotes: Vec<CashNote>,
    },
    NotifyAboutInvalidCashNote {
        from: WalletId,
        cashnote: Vec<UniquePubkey>,
    },
}

enum WalletTaskResult {
    Error {
        id: WalletId,
        err: String,
    },
    DoubleSpendSuccess {
        id: WalletId,
    },
    SendSuccess {
        id: WalletId,
        recipient_cash_notes: Vec<CashNote>,
        change_cash_note: Option<CashNote>,
        transaction: Transaction,
    },
    ReceiveSuccess {
        id: WalletId,
        received_cash_note: Vec<CashNote>,
    },
    NotifyAboutInvalidCashNoteSuccess {
        id: WalletId,
    },
}

#[derive(Debug)]
enum SpendStatus {
    Utxo,
    Spent,
    DoubleSpend,
    UtxoWithParentDoubleSpend,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum TransactionStatus {
    Valid,
    /// All the inputs have been double spent.
    DoubleSpentInputs,
}

// Just for printing things
#[derive(Debug)]
enum AttackType {
    Poison,
    DoubleSpendAllUxtoOutputs,
    DoubleSpendPartialUtxoOutputs,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, PartialOrd, Ord, Hash)]
struct WalletId(usize);

impl Display for WalletId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "WalletId({})", self.0)
    }
}

#[derive(custom_debug::Debug)]
/// The state of all the wallets and the transactions that they've performed.
struct State {
    // ========= immutable =========
    #[debug(skip)]
    /// Sender to send actions to the wallets
    action_senders: BTreeMap<WalletId, mpsc::Sender<WalletAction>>,
    /// The TempDir for each wallet. This has to be held until the end of the test.
    all_wallets: BTreeMap<WalletId, TempDir>,
    /// The main pubkeys of all the wallets.
    main_pubkeys: BTreeMap<WalletId, MainPubkey>,
    /// The map from MainPubKey to WalletId. This is used to get wallets when we only have the cashnote in hand.
    main_pubkeys_inverse: BTreeMap<MainPubkey, WalletId>,
    // ========= mutable =========
    /// The map from UniquePubkey of the cashnote to the actual cashnote and its status.
    cashnote_tracker: BTreeMap<UniquePubkey, (SpendStatus, CashNote)>,
    /// The map from WalletId to the cashnotes that it has ever received.
    cashnotes_per_wallet: BTreeMap<WalletId, Vec<UniquePubkey>>,
    /// The map from WalletId to the outbound transactions that it has ever sent.
    outbound_transactions_per_wallet: BTreeMap<WalletId, BTreeSet<Transaction>>,
    /// The status of each transaction
    transaction_status: BTreeMap<Transaction, TransactionStatus>,
}

#[derive(Debug, Default)]
struct PendingTasksTracker {
    pending_send_results: Vec<WalletId>,
    pending_notify_invalid_cashnotes_results: Vec<WalletId>,
    pending_receive_results: Vec<WalletId>,
}

/// This test aims to make sure the PUT validation of nodes are working as expected. We perform valid spends and also
/// illicit spends and finally verify them to make sure the network processed the spends as expected.
/// The illicit spends can be of these types:
/// 1. A double spend of a transaction whose outputs are partially spent / partially UTXO
/// 2. A double spend of a transcation whose outputs are all UTXO.
/// 3. Poisoning of a transaction whose outputs are all spent.
/// Todo: Double spend just 1 input spend. Currently we double spend all the inputs. Have TransactionStatus::DoubleSpentInputs(vec<inputs>)
///
/// The test works by having a main loop that sends actions to all the wallets. These are then processed by the wallets
/// in parallel. The wallets send back the results of the actions to the main loop, this is then tracked and the whole
/// cycle is repeated until the max cycles are reached.
#[tokio::test]
async fn spend_simulation() -> Result<()> {
    let _log_guards = LogBuilder::init_single_threaded_tokio_test("spend_simulation", false);

    let (client, mut state) = init_state(MAX_WALLETS).await?;

    let mut rng = rng::thread_rng();
    let (result_sender, mut result_rx) = mpsc::channel(10000);

    for (id, wallet_dir) in state.all_wallets.iter() {
        let (action_sender, action_rx) = mpsc::channel(50);
        state.action_senders.insert(*id, action_sender);
        handle_action_per_wallet(
            *id,
            wallet_dir.to_path_buf(),
            client.clone(),
            action_rx,
            result_sender.clone(),
        );
    }

    // MAIN LOOP:
    let mut cycle = 1;
    while cycle <= MAX_CYCLES {
        info!("Cycle: {cycle}/{MAX_CYCLES}");
        println!("Cycle: {cycle}/{MAX_CYCLES}");
        let mut pending_task_results = PendingTasksTracker::default();

        let iter = state
            .action_senders
            .iter()
            .map(|(id, s)| (*id, s.clone()))
            .collect_vec();
        for (our_id, action_sender) in iter {
            tokio::time::sleep(Duration::from_secs(3)).await;
            let try_performing_illicit_spend =
                rng.gen::<u32>() % ONE_IN_X_CHANCE_FOR_AN_ATTACK == 0;

            let mut illicit_spend_done = false;
            if try_performing_illicit_spend {
                if let Some((
                    input_cashnotes_to_double_spend,
                    output_cashnotes_that_are_unspendable,
                    amount,
                    attack_type,
                )) = get_cashnotes_to_double_spend(our_id, &mut state)?
                {
                    // tell wallets about the cashnotes that will become invalid after we perform the double spend.
                    if !output_cashnotes_that_are_unspendable.is_empty() {
                        info!("{our_id} is notifying wallets about invalid cashnotes: {output_cashnotes_that_are_unspendable:?}");
                        for (i, sender) in state.action_senders.iter() {
                            sender
                                .send(WalletAction::NotifyAboutInvalidCashNote {
                                    from: our_id,
                                    cashnote: output_cashnotes_that_are_unspendable.clone(),
                                })
                                .await?;
                            pending_task_results
                                .pending_notify_invalid_cashnotes_results
                                .push(*i);
                        }
                        // wait until all the wallets have received the notification. Else we'd try to spend those
                        // cashnotes while a double spend has just gone out.
                        while !pending_task_results
                            .pending_notify_invalid_cashnotes_results
                            .is_empty()
                        {
                            let result = result_rx
                                .recv()
                                .await
                                .ok_or_eyre("Senders will not be dropped")?;

                            handle_wallet_task_result(
                                &mut state,
                                result,
                                &mut pending_task_results,
                            )
                            .await?;
                        }
                    }

                    info!(
                        "{our_id} is now attempting a {attack_type:?} of {} cashnotes.",
                        input_cashnotes_to_double_spend.len()
                    );
                    println!(
                        "{our_id} is attempting a {attack_type:?} of {} cashnotes",
                        input_cashnotes_to_double_spend.len()
                    );

                    action_sender
                        .send(WalletAction::DoubleSpend {
                            input_cashnotes_to_double_spend,
                            to: (
                                amount,
                                state.main_pubkeys[&our_id],
                                DerivationIndex::random(&mut rng),
                            ),
                        })
                        .await?;
                    illicit_spend_done = true;
                }
            }
            if !illicit_spend_done {
                let recipients = get_recipients(our_id, &state);
                let recipients_len = recipients.len();
                action_sender
                    .send(WalletAction::Send {
                        recipients: recipients
                            .into_iter()
                            .map(|key| {
                                (AMOUNT_PER_RECIPIENT, key, DerivationIndex::random(&mut rng))
                            })
                            .collect_vec(),
                    })
                    .await?;
                println!("{our_id} is sending tokens to {recipients_len:?} wallets");
            }

            pending_task_results.pending_send_results.push(our_id);
            if let Ok(result) = result_rx.try_recv() {
                handle_wallet_task_result(&mut state, result, &mut pending_task_results).await?;
            }
        }

        // wait until all send && receive  tasks per cycle have been cleared
        while !pending_task_results.is_empty() {
            let result = result_rx
                .recv()
                .await
                .ok_or_eyre("Senders will not be dropped")?;

            handle_wallet_task_result(&mut state, result, &mut pending_task_results).await?;
        }

        // Since it is a tiny network, it will be overwhelemed during the verification of things and will lead to a lot
        // of Query Timeouts & huge number of pending Get requests. So let them settle.
        println!("Cycle {cycle} completed. Sleeping for 5s before next cycle.");
        tokio::time::sleep(Duration::from_secs(5)).await;

        cycle += 1;
    }

    info!("Final state: {state:?}. Sleeping before verifying wallets.");
    println!("Verifying all wallets in 10 seconds.");
    tokio::time::sleep(Duration::from_secs(10)).await;
    verify_wallets(&state, client).await?;

    Ok(())
}

fn handle_action_per_wallet(
    our_id: WalletId,
    wallet_dir: PathBuf,
    client: Client,
    mut action_rx: mpsc::Receiver<WalletAction>,
    result_sender: mpsc::Sender<WalletTaskResult>,
) {
    tokio::spawn(async move {
        let mut wallet = get_wallet(&wallet_dir);
        let mut invalid_cashnotes = BTreeSet::new();
        while let Some(action) = action_rx.recv().await {
            let result = inner_handle_action(
                our_id,
                client.clone(),
                action,
                &mut wallet,
                &mut invalid_cashnotes,
            )
            .await;
            match result {
                Ok(ok) => {
                    result_sender.send(ok).await?;
                }
                Err(err) => {
                    error!("{our_id} had error handling action : {err}");
                    result_sender
                        .send(WalletTaskResult::Error {
                            id: our_id,
                            err: format!("{err}"),
                        })
                        .await?;
                }
            }
        }
        Ok::<_, Report>(())
    });
}

async fn inner_handle_action(
    our_id: WalletId,
    client: Client,
    action: WalletAction,
    wallet: &mut HotWallet,
    invalid_cashnotes: &mut BTreeSet<UniquePubkey>,
) -> Result<WalletTaskResult> {
    match action {
        WalletAction::Send { recipients } => {
            info!("{our_id} sending to {recipients:?}");
            let (available_cash_notes, exclusive_access) = wallet.available_cash_notes()?;
            let available_cash_notes = available_cash_notes
                .into_iter()
                .filter(|(note, _)| !invalid_cashnotes.contains(&note.unique_pubkey()))
                .collect_vec();
            info!(
                "{our_id} Available CashNotes for local send: {:?}",
                available_cash_notes
            );
            let mut rng = &mut rand::rngs::OsRng;
            let derivation_index = DerivationIndex::random(&mut rng);
            let transfer = OfflineTransfer::new(
                available_cash_notes,
                recipients,
                wallet.address(),
                SpendReason::default(),
                Some((
                    wallet.key().main_pubkey(),
                    derivation_index,
                    wallet.key().derive_key(&derivation_index),
                )),
            )?;
            let recipient_cash_notes = transfer.cash_notes_for_recipient.clone();
            let change = transfer.change_cash_note.clone();
            let transaction = transfer.build_transaction();

            wallet.test_update_local_wallet(transfer, exclusive_access, true)?;

            client
                .send_spends(wallet.unconfirmed_spend_requests().iter(), true)
                .await?;
            wallet.clear_confirmed_spend_requests();
            if !wallet.unconfirmed_spend_requests().is_empty() {
                bail!("{our_id} has unconfirmed spend requests");
            }

            Ok(WalletTaskResult::SendSuccess {
                id: our_id,
                recipient_cash_notes,
                change_cash_note: change,
                transaction,
            })
        }
        // todo: we don't track the double spend tx. Track if needed.
        WalletAction::DoubleSpend {
            input_cashnotes_to_double_spend,
            to,
        } => {
            info!(
                "{our_id} double spending cash notes: {:?}",
                input_cashnotes_to_double_spend
                    .iter()
                    .map(|c| c.unique_pubkey())
                    .collect_vec()
            );
            let mut input_cashnotes_with_key =
                Vec::with_capacity(input_cashnotes_to_double_spend.len());
            for cashnote in input_cashnotes_to_double_spend {
                let derived_key = cashnote.derived_key(wallet.key())?;
                input_cashnotes_with_key.push((cashnote, Some(derived_key)));
            }
            let transfer = OfflineTransfer::new(
                input_cashnotes_with_key,
                vec![to],
                wallet.address(),
                SpendReason::default(),
                None,
            )?;
            info!("{our_id} double spending transfer: {transfer:?}");

            client
                .send_spends(transfer.all_spend_requests.iter(), false)
                .await?;

            Ok(WalletTaskResult::DoubleSpendSuccess { id: our_id })
        }
        WalletAction::ReceiveCashNotes { from, cashnotes } => {
            info!("{our_id} receiving cash note from wallet {from}");
            wallet.deposit_and_store_to_disk(&cashnotes)?;
            let our_cash_notes = cashnotes
                .into_iter()
                .filter_map(|c| {
                    // the same filter used inside the deposit fn
                    if c.derived_pubkey(&wallet.address()).is_ok() {
                        Some(c)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            Ok(WalletTaskResult::ReceiveSuccess {
                id: our_id,
                received_cash_note: our_cash_notes,
            })
        }
        WalletAction::NotifyAboutInvalidCashNote { from, cashnote } => {
            info!(
                "{our_id} received notification from {from} about invalid cashnotes: {cashnote:?}. Tracking them"
            );
            // we're just keeping track of all invalid cashnotes here, not just ours. filtering is a todo, not required for now.
            invalid_cashnotes.extend(cashnote);
            Ok(WalletTaskResult::NotifyAboutInvalidCashNoteSuccess { id: our_id })
        }
    }
}

async fn handle_wallet_task_result(
    state: &mut State,
    result: WalletTaskResult,
    pending_task_tracker: &mut PendingTasksTracker,
) -> Result<()> {
    match result {
        WalletTaskResult::DoubleSpendSuccess { id } => {
            info!("{id} received a successful double spend result");
            pending_task_tracker.send_task_completed(id);
        }
        WalletTaskResult::SendSuccess {
            id,
            recipient_cash_notes,
            change_cash_note,
            transaction,
        } => {
            info!(
                "{id} received a successful send result. Tracking the outbound transaction {:?}. Also setting status to TransactionStatus::Valid",
                transaction.hash()
            );
            pending_task_tracker.send_task_completed(id);
            match state.outbound_transactions_per_wallet.entry(id) {
                Entry::Vacant(entry) => {
                    let _ = entry.insert(BTreeSet::from([transaction.clone()]));
                }
                Entry::Occupied(entry) => {
                    entry.into_mut().insert(transaction.clone());
                }
            }
            state
                .transaction_status
                .insert(transaction.clone(), TransactionStatus::Valid);

            // mark the input cashnotes as spent
            info!("{id} marking inputs {:?} as spent", transaction.inputs);
            for input in &transaction.inputs {
                // Transaction may contains the `middle payment`
                if let Some((status, _cashnote)) =
                    state.cashnote_tracker.get_mut(&input.unique_pubkey)
                {
                    *status = SpendStatus::Spent;
                }
            }

            // track the change cashnote that is stored by our wallet.
            if let Some(change) = change_cash_note {
                info!(
                    "{id} tracking change cash note {} as UTXO",
                    change.unique_pubkey()
                );
                state
                    .cashnotes_per_wallet
                    .get_mut(&id)
                    .ok_or_eyre("Wallet should be present")?
                    .push(change.unique_pubkey());
                let result = state
                    .cashnote_tracker
                    .insert(change.unique_pubkey(), (SpendStatus::Utxo, change));
                if result.is_some() {
                    bail!("{id} received a new cash note that was already tracked");
                }
            }

            info!("{id}, sending the recipient cash notes to the other wallets");
            // send the recipient cash notes to the wallets
            for cashnote in recipient_cash_notes {
                let recipient_id = state
                    .main_pubkeys_inverse
                    .get(cashnote.main_pubkey())
                    .ok_or_eyre("Recipient for cashnote not found")?;
                let sender = state
                    .action_senders
                    .get(recipient_id)
                    .ok_or_eyre("Recipient action sender not found")?;
                sender
                    .send(WalletAction::ReceiveCashNotes {
                        from: id,
                        cashnotes: vec![cashnote],
                    })
                    .await?;
                // track the task
                pending_task_tracker
                    .pending_receive_results
                    .push(*recipient_id);
            }
        }
        WalletTaskResult::ReceiveSuccess {
            id,
            received_cash_note,
        } => {
            info!(
                "{id} received cashnotes successfully. Marking {:?} as UTXO",
                received_cash_note
                    .iter()
                    .map(|c| c.unique_pubkey())
                    .collect_vec()
            );
            pending_task_tracker.receive_task_completed(id);
            for cashnote in received_cash_note {
                let unique_pubkey = cashnote.unique_pubkey();
                let result = state
                    .cashnote_tracker
                    .insert(unique_pubkey, (SpendStatus::Utxo, cashnote));
                if result.is_some() {
                    bail!("{id} received a new cash note that was already tracked");
                }

                match state.cashnotes_per_wallet.entry(id) {
                    Entry::Vacant(_) => {
                        bail!("{id} should not be empty, something went wrong.")
                    }
                    Entry::Occupied(entry) => entry.into_mut().push(unique_pubkey),
                }
            }
        }
        WalletTaskResult::NotifyAboutInvalidCashNoteSuccess { id } => {
            info!("{id} received notification about invalid cashnotes successfully. Marking task as completed.");
            pending_task_tracker.notify_invalid_cashnote_task_completed(id);
        }
        WalletTaskResult::Error { id, err } => {
            error!("{id} had an error: {err}");
            info!("state: {state:?}");
            bail!("{id} had an error: {err}");
        }
    }
    Ok(())
}

async fn verify_wallets(state: &State, client: Client) -> Result<()> {
    for (id, spends) in state.cashnotes_per_wallet.iter() {
        println!("Verifying wallet {id}");
        info!("{id} verifying {} spends", spends.len());
        let mut wallet = get_wallet(state.all_wallets.get(id).expect("Wallet not found"));
        let (available_cash_notes, _lock) = wallet.available_cash_notes()?;
        for (num, spend) in spends.iter().enumerate() {
            let (status, _cashnote) = state
                .cashnote_tracker
                .get(spend)
                .ok_or_eyre("Something went wrong. Spend not tracked")?;
            info!("{id} verifying status of spend number({num:?}): {spend:?} : {status:?}");
            match status {
                SpendStatus::Utxo => {
                    // TODO: with the new spend struct requiring `middle payment`
                    //       the transaction no longer covers all spends to be tracked
                    //       leaving the chance the Spend retain as UTXO even got spent properly
                    //       Currently just log it, leave for further work of replace transaction
                    //       with a properly formatted new instance.
                    if !available_cash_notes
                        .iter()
                        .find(|(c, _)| &c.unique_pubkey() == spend)
                        .ok_or_eyre("UTXO not found in wallet")?;
                    let addr = SpendAddress::from_unique_pubkey(spend);
                    let result = client.peek_a_spend(addr).await;
                    assert_matches!(
                        result,
                        Err(sn_client::Error::Network(NetworkError::GetRecordError(
                            GetRecordError::RecordNotFound
                        )))
                    );
                }
                SpendStatus::Spent => {
                    let addr = SpendAddress::from_unique_pubkey(spend);
                    let _spend = client.get_spend_from_network(addr).await?;
                }
                SpendStatus::DoubleSpend => {
                    let addr = SpendAddress::from_unique_pubkey(spend);
                    match client.get_spend_from_network(addr).await {
                        Err(sn_client::Error::Network(NetworkError::DoubleSpendAttempt(_))) => {
                            info!("Poisoned spend {addr:?} failed with query attempt");
                        }
                        other => {
                            warn!("Poisoned spend {addr:?} got unexpected query attempt {other:?}")
                        }
                    }
                }
                SpendStatus::UtxoWithParentDoubleSpend => {
                    // should not have been spent (we're tracking this internally in the test)
                    available_cash_notes
                        .iter()
                        .find(|(c, _)| &c.unique_pubkey() == spend)
                        .ok_or_eyre("UTXO not found in wallet")?;
                    let addr = SpendAddress::from_unique_pubkey(spend);
                    let result = client.peek_a_spend(addr).await;
                    assert_matches!(
                        result,
                        Err(sn_client::Error::Network(NetworkError::GetRecordError(
                            GetRecordError::RecordNotFound
                        )))
                    );
                }
            }
            info!("{id} successfully verified spend number({num:?}): {spend:?} : {status:?}");
        }
    }
    println!("All wallets verified successfully");
    Ok(())
}

/// Create `count` number of wallets and fund them all with equal amounts of tokens.
/// Return the client and the states of the wallets.
async fn init_state(count: usize) -> Result<(Client, State)> {
    let mut state = State {
        all_wallets: BTreeMap::new(),
        main_pubkeys: BTreeMap::new(),
        action_senders: BTreeMap::new(),
        main_pubkeys_inverse: BTreeMap::new(),
        cashnote_tracker: BTreeMap::new(),
        cashnotes_per_wallet: BTreeMap::new(),
        outbound_transactions_per_wallet: BTreeMap::new(),
        transaction_status: BTreeMap::new(),
    };

    for i in 0..count {
        let wallet_dir = TempDir::new()?;
        let i = WalletId(i);
        state
            .main_pubkeys
            .insert(i, get_wallet(wallet_dir.path()).address());
        state
            .main_pubkeys_inverse
            .insert(get_wallet(wallet_dir.path()).address(), i);
        state.all_wallets.insert(i, wallet_dir);
    }

    let first_wallet_dir = TempDir::new()?;
    let (client, mut first_wallet) = get_client_and_funded_wallet(first_wallet_dir.path()).await?;

    let amount = NanoTokens::from(first_wallet.balance().as_nano() / MAX_WALLETS as u64);
    info!(
        "Funding all the wallets of len: {} each with {amount} tokens",
        state.main_pubkeys.len(),
    );

    let mut rng = rng::thread_rng();
    let reason = SpendReason::default();

    let mut recipients = Vec::new();
    for address in state.main_pubkeys.values() {
        let to = (amount, *address, DerivationIndex::random(&mut rng));
        recipients.push(to);
    }

    let (available_cash_notes, _lock) = first_wallet.available_cash_notes()?;

    let transfer = OfflineTransfer::new(
        available_cash_notes,
        recipients,
        first_wallet.address(),
        reason.clone(),
        None,
    )?;

    info!("Sending transfer for all wallets and verifying them");
    client
        .send_spends(transfer.all_spend_requests.iter(), true)
        .await?;

    for (id, address) in state.main_pubkeys.iter() {
        let mut wallet = get_wallet(state.all_wallets.get(id).expect("Id should be present"));
        wallet.deposit_and_store_to_disk(&transfer.cash_notes_for_recipient)?;
        trace!(
            "{id} with main_pubkey: {address:?} has balance: {}",
            wallet.balance()
        );
        assert_eq!(wallet.balance(), amount);

        let (available_cash_notes, _lock) = wallet.available_cash_notes()?;

        for (cashnote, _) in available_cash_notes {
            state.cashnote_tracker.insert(
                cashnote.unique_pubkey,
                (SpendStatus::Utxo, cashnote.clone()),
            );
            match state.cashnotes_per_wallet.entry(*id) {
                Entry::Vacant(entry) => {
                    let _ = entry.insert(vec![cashnote.unique_pubkey]);
                }
                Entry::Occupied(entry) => entry.into_mut().push(cashnote.unique_pubkey),
            }
        }
    }

    Ok((client, state))
}

/// Returns random recipients to send tokens to.
/// Random recipient of random lengths are chosen.
fn get_recipients(our_id: WalletId, state: &State) -> Vec<(MainPubkey, WalletId)> {
    let mut recipients = Vec::new();

    let mut random_number = our_id;
    while random_number == our_id {
        random_number = WalletId(rand::thread_rng().gen_range(0..state.main_pubkeys.len()));
    }
    recipients.push((state.main_pubkeys[&random_number], random_number));

    while random_number.0 % 4 != 0 {
        random_number = WalletId(rand::thread_rng().gen_range(0..state.main_pubkeys.len()));
        if random_number != our_id
            && !recipients
                .iter()
                .any(|(_, existing_id)| *existing_id == random_number)
        {
            recipients.push((state.main_pubkeys[&random_number], random_number));
        }
    }

    info!("{our_id} the recipients for send are: {recipients:?}");
    recipients
}

/// Checks our state and tries to perform double spends in these order:
/// Poison old spend whose outputs are all spent.
/// Double spend a transaction whose outputs are partially spent / partially UTXO
/// Double spend a transaction whose outputs are all UTXO.
/// Returns the set of input cashnotes to double spend and the keys of the output cashnotes that will be unspendable
/// after the attack.
#[allow(clippy::type_complexity)]
fn get_cashnotes_to_double_spend(
    our_id: WalletId,
    state: &mut State,
) -> Result<Option<(Vec<CashNote>, Vec<UniquePubkey>, NanoTokens, AttackType)>> {
    let mut rng = rand::thread_rng();
    let mut attack_type;
    let mut cashnotes_to_double_spend;

    cashnotes_to_double_spend = get_random_transaction_to_poison(our_id, state, &mut rng)?;
    attack_type = AttackType::Poison;

    if cashnotes_to_double_spend.is_none() {
        cashnotes_to_double_spend =
            get_random_transaction_with_partially_spent_output(our_id, state, &mut rng)?;
        attack_type = AttackType::DoubleSpendPartialUtxoOutputs;
    }
    if cashnotes_to_double_spend.is_none() {
        cashnotes_to_double_spend =
            get_random_transaction_with_all_unspent_output(our_id, state, &mut rng)?;
        attack_type = AttackType::DoubleSpendAllUxtoOutputs;
    }

    if let Some((cashnotes_to_double_spend, output_cash_notes_that_are_unspendable)) =
        cashnotes_to_double_spend
    {
        //gotta make sure the amount adds up to the input, else not all cashnotes will be utilized
        let mut input_total_amount = 0;
        for cashnote in &cashnotes_to_double_spend {
            input_total_amount += cashnote.value()?.as_nano();
        }
        return Ok(Some((
            cashnotes_to_double_spend,
            output_cash_notes_that_are_unspendable,
            NanoTokens::from(input_total_amount),
            attack_type,
        )));
    }

    Ok(None)
}

/// Returns the input cashnotes of a random transaction whose: outputs are all spent.
/// This also modified the status of the cashnote.
fn get_random_transaction_to_poison(
    our_id: WalletId,
    state: &mut State,
    rng: &mut rand::rngs::ThreadRng,
) -> Result<Option<(Vec<CashNote>, Vec<UniquePubkey>)>> {
    let Some(our_transactions) = state.outbound_transactions_per_wallet.get(&our_id) else {
        info!("{our_id} has no outbound transactions yet. Skipping double spend");
        return Ok(None);
    };

    if our_transactions.is_empty() {
        info!("{our_id} has no outbound transactions yet. Skipping double spend");
        return Ok(None);
    }

    // A spend / transaction is poisonable if all of its outputs are already spent.
    let mut poisonable_tx = Vec::new();
    for tx in our_transactions {
        let tx_status = state
            .transaction_status
            .get(tx)
            .ok_or_eyre("The tx should be present")?;
        // This tx has already been attacked. Skip.
        if tx_status == &TransactionStatus::DoubleSpentInputs {
            continue;
        }
        let mut utxo_found = false;
        for output in &tx.outputs {
            let (status, _) = state
                .cashnote_tracker
                .get(output.unique_pubkey())
                .ok_or_eyre(format!(
                    "Output {} not found in cashnote tracker",
                    output.unique_pubkey()
                ))?;

            if let SpendStatus::Utxo = *status {
                utxo_found = true;
                break;
            }
        }
        if !utxo_found {
            poisonable_tx.push(tx);
        }
    }
    if !poisonable_tx.is_empty() {
        let random_tx = poisonable_tx
            .into_iter()
            .choose(rng)
            .ok_or_eyre("Cannot choose a random tx")?;
        // update the tx status
        *state
            .transaction_status
            .get_mut(random_tx)
            .ok_or_eyre("The tx should be present")? = TransactionStatus::DoubleSpentInputs;

        info!(
            "{our_id} is attempting to double spend a transaction {:?} whose outputs all ALL spent. Setting tx status to TransactionStatus::DoubleSpentInputs", random_tx.hash()
        );
        info!(
            "{our_id} is marking inputs {:?} as DoubleSpend",
            random_tx
                .inputs
                .iter()
                .map(|i| i.unique_pubkey())
                .collect_vec()
        );

        let mut cashnotes_to_double_spend = Vec::new();
        for input in &random_tx.inputs {
            let (status, cashnote) = state
                .cashnote_tracker
                .get_mut(&input.unique_pubkey)
                .ok_or_eyre("Input spend not tracked")?;
            *status = SpendStatus::DoubleSpend;
            cashnotes_to_double_spend.push(cashnote.clone());
        }

        return Ok(Some((cashnotes_to_double_spend, vec![])));
    }
    Ok(None)
}

/// Returns the input cashnotes of a random transaction whose: outputs are partially spent / partially UTXO.
/// Also returns the uniquepub key of output UTXOs  that will be unspendable after the attack. This info is sent to
/// each wallet, so that they don't try to spend these outputs.
/// This also modified the status of the cashnote.
fn get_random_transaction_with_partially_spent_output(
    our_id: WalletId,
    state: &mut State,
    rng: &mut rand::rngs::ThreadRng,
) -> Result<Option<(Vec<CashNote>, Vec<UniquePubkey>)>> {
    let Some(our_transactions) = state.outbound_transactions_per_wallet.get(&our_id) else {
        info!("{our_id} has no outbound transactions yet. Skipping double spend");
        return Ok(None);
    };

    if our_transactions.is_empty() {
        info!("{our_id} has no outbound transactions yet. Skipping double spend");
        return Ok(None);
    }

    // The list of transactions that have outputs that are partially spent / partially UTXO.
    let mut double_spendable_tx = Vec::new();
    for tx in our_transactions {
        let tx_status = state
            .transaction_status
            .get(tx)
            .ok_or_eyre("The tx should be present")?;
        // This tx has already been attacked. Skip.
        if tx_status == &TransactionStatus::DoubleSpentInputs {
            continue;
        }
        let mut utxo_found = false;
        let mut spent_output_found = false;
        let mut change_cashnote_found = false;
        for output in &tx.outputs {
            let (status, cashnote) = state
                .cashnote_tracker
                .get(output.unique_pubkey())
                .ok_or_eyre(format!(
                    "Output {} not found in cashnote tracker",
                    output.unique_pubkey()
                ))?;

            match status {
                SpendStatus::Utxo => {
                    // skip if the cashnote is the change. The test can't progress if we make the change unspendable.
                    if cashnote.value()? > NanoTokens::from(AMOUNT_PER_RECIPIENT.as_nano()*10) {
                        change_cashnote_found = true;
                        break;
                    }
                    utxo_found = true;
                },
                SpendStatus::UtxoWithParentDoubleSpend => bail!("UtxoWithParentDoubleSpend should not be present here. We skip txs that has been attacked"),
                SpendStatus::Spent
                // DoubleSpend can be present. TransactionStatus::DoubleSpentInputs means that inputs are double spent, we skip those.
                // So the output with DoubleSpend will be present here.
                | SpendStatus::DoubleSpend => spent_output_found = true,

            }
        }
        if change_cashnote_found {
            continue;
        } else if utxo_found && spent_output_found {
            double_spendable_tx.push(tx);
        }
    }

    if !double_spendable_tx.is_empty() {
        let random_tx = double_spendable_tx
            .into_iter()
            .choose(rng)
            .ok_or_eyre("Cannot choose a random tx")?;
        // update the tx status
        *state
            .transaction_status
            .get_mut(random_tx)
            .ok_or_eyre("The tx should be present")? = TransactionStatus::DoubleSpentInputs;

        info!("{our_id} is attempting to double spend a transaction {:?} whose outputs are partially spent. Setting tx status to TransactionStatus::DoubleSpentInputs", random_tx.hash());
        info!(
            "{our_id} is marking inputs {:?} as DoubleSpend",
            random_tx
                .inputs
                .iter()
                .map(|i| i.unique_pubkey())
                .collect_vec()
        );

        let mut cashnotes_to_double_spend = Vec::new();
        for input in &random_tx.inputs {
            let (status, cashnote) = state
                .cashnote_tracker
                .get_mut(&input.unique_pubkey)
                .ok_or_eyre("Input spend not tracked")?;
            *status = SpendStatus::DoubleSpend;
            cashnotes_to_double_spend.push(cashnote.clone());
        }

        let mut marked_output_as_cashnotes_unspendable_utxo = Vec::new();
        for output in &random_tx.outputs {
            let (status, cashnote) = state
                .cashnote_tracker
                .get_mut(output.unique_pubkey())
                .ok_or_eyre("Output spend not tracked")?;
            if let SpendStatus::Utxo = *status {
                *status = SpendStatus::UtxoWithParentDoubleSpend;
                marked_output_as_cashnotes_unspendable_utxo.push(cashnote.unique_pubkey);
            }
        }
        info!(
            "{our_id} is marking some outputs {:?} as UtxoWithParentDoubleSpend",
            marked_output_as_cashnotes_unspendable_utxo
        );

        return Ok(Some((
            cashnotes_to_double_spend,
            marked_output_as_cashnotes_unspendable_utxo,
        )));
    }

    Ok(None)
}

/// Returns the input cashnotes of a random transaction whose: outputs are all UTXO.
/// Also returns the uniquepub key of output UTXOs  that will be unspendable after the attack. This info is sent to
/// each wallet, so that they don't try to spend these outputs.
/// This also modified the status of the cashnote.
fn get_random_transaction_with_all_unspent_output(
    our_id: WalletId,
    state: &mut State,
    rng: &mut rand::rngs::ThreadRng,
) -> Result<Option<(Vec<CashNote>, Vec<UniquePubkey>)>> {
    let Some(our_transactions) = state.outbound_transactions_per_wallet.get(&our_id) else {
        info!("{our_id} has no outbound transactions yet. Skipping double spend");
        return Ok(None);
    };

    if our_transactions.is_empty() {
        info!("{our_id} has no outbound transactions yet. Skipping double spend");
        return Ok(None);
    }

    let mut double_spendable_tx = Vec::new();
    for tx in our_transactions {
        let tx_status = state
            .transaction_status
            .get(tx)
            .ok_or_eyre("The tx should be present")?;
        if tx_status == &TransactionStatus::DoubleSpentInputs {
            continue;
        }
        let mut all_utxos = true;
        let mut change_cashnote_found = false;
        for output in &tx.outputs {
            let (status, cashnote) = state
                .cashnote_tracker
                .get(output.unique_pubkey())
                .ok_or_eyre(format!(
                    "Output {} not found in cashnote tracker",
                    output.unique_pubkey()
                ))?;

            match status {
                SpendStatus::Utxo => {
                    // skip if the cashnote is the change. The test can't progress if we make the change unspendable.
                    if cashnote.value()? > NanoTokens::from(AMOUNT_PER_RECIPIENT.as_nano()*10) {
                        change_cashnote_found = true;
                        break;
                    }
                }
                SpendStatus::UtxoWithParentDoubleSpend => bail!("UtxoWithParentDoubleSpend should not be present here. We skip txs that has been attacked"),
                _ => {
                    all_utxos = false;
                    break;
                }
            }
        }
        if change_cashnote_found {
            continue;
        } else if all_utxos {
            double_spendable_tx.push(tx);
        }
    }

    if !double_spendable_tx.is_empty() {
        let random_tx = double_spendable_tx
            .into_iter()
            .choose(rng)
            .ok_or_eyre("Cannot choose a random tx")?;
        // update the tx status
        *state
            .transaction_status
            .get_mut(random_tx)
            .ok_or_eyre("The tx should be present")? = TransactionStatus::DoubleSpentInputs;

        info!("{our_id} is attempting to double spend a transaction {:?} whose outputs are all UTXO. Setting tx status to TransactionStatus::DoubleSpentInputs", random_tx.hash());
        info!(
            "{our_id} is marking inputs {:?} as DoubleSpend",
            random_tx
                .inputs
                .iter()
                .map(|i| i.unique_pubkey())
                .collect_vec()
        );

        let mut cashnotes_to_double_spend = Vec::new();
        for input in &random_tx.inputs {
            let (status, cashnote) = state
                .cashnote_tracker
                .get_mut(&input.unique_pubkey)
                .ok_or_eyre("Input spend not tracked")?;
            *status = SpendStatus::DoubleSpend;
            cashnotes_to_double_spend.push(cashnote.clone());
        }

        let mut marked_output_cashnotes_as_unspendable_utxo = Vec::new();
        for output in &random_tx.outputs {
            let (status, cashnote) = state
                .cashnote_tracker
                .get_mut(output.unique_pubkey())
                .ok_or_eyre("Output spend not tracked")?;
            *status = SpendStatus::UtxoWithParentDoubleSpend;
            marked_output_cashnotes_as_unspendable_utxo.push(cashnote.unique_pubkey);
        }
        info!(
            "{our_id} is marking all outputs {:?} as UtxoWithParentDoubleSpend",
            marked_output_cashnotes_as_unspendable_utxo
        );

        return Ok(Some((
            cashnotes_to_double_spend,
            marked_output_cashnotes_as_unspendable_utxo,
        )));
    }

    Ok(None)
}

impl PendingTasksTracker {
    fn is_empty(&self) -> bool {
        self.pending_send_results.is_empty()
            && self.pending_receive_results.is_empty()
            && self.pending_notify_invalid_cashnotes_results.is_empty()
    }

    fn send_task_completed(&mut self, id: WalletId) {
        let pos = self
            .pending_send_results
            .iter()
            .position(|x| *x == id)
            .unwrap_or_else(|| panic!("Send task for {id} was not found "));
        self.pending_send_results.remove(pos);
    }

    fn receive_task_completed(&mut self, id: WalletId) {
        let pos = self
            .pending_receive_results
            .iter()
            .position(|x| *x == id)
            .unwrap_or_else(|| panic!("Receive task for {id} was not found "));
        self.pending_receive_results.remove(pos);
    }

    fn notify_invalid_cashnote_task_completed(&mut self, id: WalletId) {
        let pos = self
            .pending_notify_invalid_cashnotes_results
            .iter()
            .position(|x| *x == id)
            .unwrap_or_else(|| panic!("Notify invalid cashnote task for {id} was not found "));
        self.pending_notify_invalid_cashnotes_results.remove(pos);
    }
}
