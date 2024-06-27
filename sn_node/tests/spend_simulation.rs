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
use rand::Rng;
use sn_client::Client;
use sn_logging::LogBuilder;
use sn_transfers::{
    rng, CashNote, DerivationIndex, HotWallet, MainPubkey, NanoTokens, OfflineTransfer,
    SpendAddress, SpendReason, Transaction, UniquePubkey,
};
use std::{
    collections::{btree_map::Entry, BTreeMap, BTreeSet},
    path::PathBuf,
    time::Duration,
};
use tokio::sync::mpsc;
use tracing::*;

const MAX_WALLETS: usize = 30;
const MAX_CYCLES: usize = 5;
const AMOUNT_PER_RECIPIENT: NanoTokens = NanoTokens::from(1000);

enum WalletAction {
    Send {
        recipients: Vec<(NanoTokens, MainPubkey)>,
    },
    ReceiveCashNotes(Vec<CashNote>),
}

enum WalletTaskResult {
    Error {
        id: usize,
        err: String,
    },
    SendSuccess {
        id: usize,
        recipient_cash_notes: Vec<CashNote>,
        transaction: Transaction,
    },
    ReceiveSuccess {
        id: usize,
        received_cash_note: Vec<UniquePubkey>,
    },
}

#[derive(Debug)]
enum SpendStatus {
    Utxo,
    Spent { transaction: Transaction },
}

#[derive(custom_debug::Debug)]
struct State {
    // immutable
    #[debug(skip)]
    action_senders: BTreeMap<usize, mpsc::Sender<WalletAction>>,
    all_wallets: BTreeMap<usize, TempDir>,
    main_pubkeys: BTreeMap<usize, MainPubkey>,
    main_pubkeys_inverse: BTreeMap<MainPubkey, usize>,
    // mut
    cashnote_tracker: BTreeMap<UniquePubkey, SpendStatus>,
    cashnotes_per_wallet: BTreeMap<usize, Vec<UniquePubkey>>,
}

#[derive(Debug, Default)]
struct PendingTasksTracker {
    pending_send_results: Vec<usize>,
    pending_receive_results: Vec<usize>,
}

#[tokio::test]
async fn cash_note_transfer_double_spend_fail() -> Result<()> {
    let _log_guards = LogBuilder::init_single_threaded_tokio_test("spend_simulation", true);

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
    let mut cycle = 0;
    while cycle < MAX_CYCLES {
        let mut pending_task_results = PendingTasksTracker::default();

        let iter = state
            .action_senders
            .iter()
            .map(|(id, s)| (*id, s.clone()))
            .collect_vec();
        for (id, action_sender) in iter {
            let should_attack = rng.gen::<u32>() % 10 == 0;

            let recipients = get_recipients(id, &state.main_pubkeys);
            action_sender
                .send(WalletAction::Send {
                    recipients: recipients
                        .into_iter()
                        .map(|key| (AMOUNT_PER_RECIPIENT, key))
                        .collect_vec(),
                })
                .await?;
            pending_task_results.pending_send_results.push(id);

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

        cycle += 1;
    }

    info!("Final state: {state:?}. Sleeping before verifying wallets.");
    tokio::time::sleep(Duration::from_secs(3)).await;
    verify_wallets(&state, client).await?;

    Ok(())
}

fn handle_action_per_wallet(
    our_id: usize,
    wallet_dir: PathBuf,
    client: Client,
    mut action_rx: mpsc::Receiver<WalletAction>,
    result_sender: mpsc::Sender<WalletTaskResult>,
) {
    tokio::spawn(async move {
        let mut wallet = get_wallet(&wallet_dir);
        while let Some(action) = action_rx.recv().await {
            let result = inner_handle_action(our_id, client.clone(), action, &mut wallet).await;
            match result {
                Ok(ok) => {
                    result_sender.send(ok).await?;
                }
                Err(err) => {
                    error!("TestWallet {our_id} had error handling action : {err}");
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
    our_id: usize,
    client: Client,
    action: WalletAction,
    wallet: &mut HotWallet,
) -> Result<WalletTaskResult> {
    match action {
        WalletAction::Send { recipients } => {
            info!("TestWallet {our_id} sending to {recipients:?}");
            let recipient_cash_notes = wallet.local_send(recipients, None)?;
            // the parent tx for all the recipient cash notes should be the same.
            let transaction = recipient_cash_notes
                .iter()
                .map(|c| c.parent_tx.clone())
                .collect::<BTreeSet<_>>();
            if transaction.len() != 1 {
                bail!("TestWallet {our_id}: Transactions should have the same parent tx");
            }

            client
                .send_spends(wallet.unconfirmed_spend_requests().iter(), true)
                .await?;
            wallet.clear_confirmed_spend_requests();
            if !wallet.unconfirmed_spend_requests().is_empty() {
                bail!("TestWallet {our_id} has unconfirmed spend requests");
            }

            Ok(WalletTaskResult::SendSuccess {
                id: our_id,
                recipient_cash_notes,
                transaction: transaction
                    .into_iter()
                    .next()
                    .expect("Should've bailed earlier"),
            })
        }
        WalletAction::ReceiveCashNotes(cash_notes) => {
            info!("TestWallet {our_id} receiving cash note");
            wallet.deposit_and_store_to_disk(&cash_notes)?;
            let our_cash_notes = cash_notes
                .iter()
                .filter_map(|c| {
                    if c.derived_pubkey(&wallet.address()).is_ok() {
                        Some(c.unique_pubkey())
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
    }
}

async fn handle_wallet_task_result(
    state: &mut State,
    result: WalletTaskResult,
    pending_task_tracker: &mut PendingTasksTracker,
) -> Result<()> {
    match result {
        WalletTaskResult::SendSuccess {
            id,
            recipient_cash_notes,
            transaction,
        } => {
            info!("TestWallet {id} received a successful send result");
            pending_task_tracker.send_task_completed(id);

            // mark the input cashnotes as spent
            info!(
                "Wallet {id} marking inputs {:?} as spent",
                transaction.inputs
            );
            for input in &transaction.inputs {
                let status = state
                    .cashnote_tracker
                    .get_mut(&input.unique_pubkey)
                    .ok_or_eyre("Input spend not tracked")?;
                *status = SpendStatus::Spent {
                    transaction: transaction.clone(),
                };
            }

            // track the change cashnote that is stored by our wallet. The output that is not part of the recipient_cash_notes
            // is the change cashnote.
            // Change is already deposited in the wallet. Just track it here
            let mut change = None;
            for output in &transaction.outputs {
                if !recipient_cash_notes
                    .iter()
                    .any(|c| &c.unique_pubkey() == output.unique_pubkey())
                {
                    if change.is_some() {
                        bail!("TestWallet {id} has more than one change cash note");
                    }
                    change = Some(*output.unique_pubkey());
                }
            }
            if let Some(change) = change {
                let result = state.cashnote_tracker.insert(change, SpendStatus::Utxo);
                if result.is_some() {
                    bail!("TestWallet {id} received a new cash note that was already tracked");
                }
                state
                    .cashnotes_per_wallet
                    .get_mut(&id)
                    .ok_or_eyre("Wallet should be present")?
                    .push(change);
            }

            info!("TestWallet {id}, sending the recipient cash notes to the other wallets");
            // send the recipient cash notes to the wallets
            for cashnote in recipient_cash_notes {
                let recipient_id = state
                    .main_pubkeys_inverse
                    .get(cashnote.main_pubkey())
                    .ok_or_eyre("Recipient for cashnote not found")?;
                state
                    .action_senders
                    .get(recipient_id)
                    .ok_or_eyre("Recipient action sender not found")?
                    .send(WalletAction::ReceiveCashNotes(vec![cashnote]))
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
            info!("TestWallet {id} received cashnotes successfully. Marking {received_cash_note:?} as UTXO");
            pending_task_tracker.receive_task_completed(id);
            for cash_note in received_cash_note {
                let result = state.cashnote_tracker.insert(cash_note, SpendStatus::Utxo);
                if result.is_some() {
                    bail!("TestWallet {id} received a new cash note that was already tracked");
                }

                match state.cashnotes_per_wallet.entry(id) {
                    Entry::Vacant(_) => {
                        bail!("TestWallet {id} should not be empty, something went wrong.")
                    }
                    Entry::Occupied(entry) => entry.into_mut().push(cash_note),
                }
            }
        }
        WalletTaskResult::Error { id, err } => {
            error!("TestWallet {id} had an error: {err}");
            bail!("TestWallet {id} had an error: {err}");
        }
    }
    Ok(())
}

async fn verify_wallets(state: &State, client: Client) -> Result<()> {
    info!("Verifying all wallets");
    for (id, spends) in state.cashnotes_per_wallet.iter() {
        info!("TestWallet {id} verifying {} spends", spends.len());
        let mut wallet = get_wallet(state.all_wallets.get(id).expect("Wallet not found"));
        let (available_cash_notes, _lock) = wallet.available_cash_notes()?;
        for spend in spends {
            let status = state
                .cashnote_tracker
                .get(spend)
                .ok_or_eyre("Something went wrong. Spend not tracked")?;
            match status {
                SpendStatus::Utxo => {
                    available_cash_notes
                        .iter()
                        .find(|(c, _)| &c.unique_pubkey() == spend)
                        .ok_or_eyre("UTXO not found in wallet")?;
                }
                SpendStatus::Spent { transaction } => {
                    for input in &transaction.inputs {
                        let addr = SpendAddress::from_unique_pubkey(input.unique_pubkey());
                        let _spend = client.get_spend_from_network(addr).await?;
                    }
                }
            }
        }
    }
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
    };

    for i in 0..count {
        let wallet_dir = TempDir::new()?;
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
    )?;

    info!("Sending transfer for all wallets and verifying them");
    client
        .send_spends(transfer.all_spend_requests.iter(), true)
        .await?;

    for (id, address) in state.main_pubkeys.iter() {
        let mut wallet = get_wallet(state.all_wallets.get(id).expect("Id should be present"));
        wallet.deposit_and_store_to_disk(&transfer.cash_notes_for_recipient)?;
        trace!(
            "Wallet {id} with main_pubkey: {address:?} has balance: {}",
            wallet.balance()
        );
        assert_eq!(wallet.balance(), amount);

        let (available_cash_notes, _lock) = wallet.available_cash_notes()?;

        for (cashnote, _) in available_cash_notes {
            state
                .cashnote_tracker
                .insert(cashnote.unique_pubkey, SpendStatus::Utxo);
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

fn get_recipients(our_id: usize, all_address: &BTreeMap<usize, MainPubkey>) -> Vec<MainPubkey> {
    let mut recipients = Vec::new();

    let mut random_number = our_id;
    while random_number != our_id {
        random_number = rand::thread_rng().gen_range(0..all_address.len());
    }
    recipients.push(all_address[&random_number]);

    while random_number % 3 != 0 {
        random_number = rand::thread_rng().gen_range(0..all_address.len());
        if random_number != our_id {
            recipients.push(all_address[&random_number]);
        }
    }

    info!("Recipients from id: {our_id} are: {recipients:?}");
    recipients
}

impl PendingTasksTracker {
    fn is_empty(&self) -> bool {
        self.pending_send_results.is_empty() && self.pending_receive_results.is_empty()
    }

    fn send_task_completed(&mut self, id: usize) {
        let pos = self
            .pending_send_results
            .iter()
            .position(|x| *x == id)
            .unwrap_or_else(|| panic!("Send task for {id} was not found "));
        self.pending_send_results.remove(pos);
    }

    fn receive_task_completed(&mut self, id: usize) {
        let pos = self
            .pending_receive_results
            .iter()
            .position(|x| *x == id)
            .unwrap_or_else(|| panic!("Receive task for {id} was not found "));
        self.pending_receive_results.remove(pos);
    }
}
