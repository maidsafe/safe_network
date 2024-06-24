// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod common;

use assert_fs::TempDir;
use assert_matches::assert_matches;
use common::client::{get_client_and_funded_wallet, get_wallet};
use eyre::Result;
use sn_logging::LogBuilder;
use sn_transfers::{
    get_genesis_sk, rng, DerivationIndex, HotWallet, NanoTokens, OfflineTransfer, SpendReason,
    WalletError, GENESIS_CASHNOTE,
};
use std::time::Duration;
use tracing::*;

#[tokio::test]
async fn cash_note_transfer_double_spend_fail() -> Result<()> {
    let _log_guards = LogBuilder::init_single_threaded_tokio_test("double_spend", true);
    // create 1 wallet add money from faucet
    let first_wallet_dir = TempDir::new()?;

    let (client, mut first_wallet) = get_client_and_funded_wallet(first_wallet_dir.path()).await?;
    let first_wallet_balance = first_wallet.balance().as_nano();

    // create wallet 2 and 3 to receive money from 1
    let second_wallet_dir = TempDir::new()?;
    let second_wallet = get_wallet(second_wallet_dir.path());
    assert_eq!(second_wallet.balance(), NanoTokens::zero());
    let third_wallet_dir = TempDir::new()?;
    let third_wallet = get_wallet(third_wallet_dir.path());
    assert_eq!(third_wallet.balance(), NanoTokens::zero());

    // manually forge two transfers of the same source
    let amount = NanoTokens::from(first_wallet_balance / 3);
    let to1 = first_wallet.address();
    let to2 = second_wallet.address();
    let to3 = third_wallet.address();

    let (some_cash_notes, _exclusive_access) = first_wallet.available_cash_notes()?;
    let same_cash_notes = some_cash_notes.clone();

    let mut rng = rng::thread_rng();

    let reason = SpendReason::default();
    let to2_unique_key = (amount, to2, DerivationIndex::random(&mut rng));
    let to3_unique_key = (amount, to3, DerivationIndex::random(&mut rng));

    let transfer_to_2 =
        OfflineTransfer::new(some_cash_notes, vec![to2_unique_key], to1, reason.clone()).unwrap();
    let transfer_to_3 =
        OfflineTransfer::new(same_cash_notes, vec![to3_unique_key], to1, reason).unwrap();

    // send both transfers to the network
    // upload won't error out, only error out during verification.
    info!("Sending both transfers to the network...");
    let res = client
        .send_spends(transfer_to_2.all_spend_requests.iter(), false)
        .await;
    assert!(res.is_ok());
    let res = client
        .send_spends(transfer_to_3.all_spend_requests.iter(), false)
        .await;
    assert!(res.is_ok());

    // check the CashNotes, it should fail
    info!("Verifying the transfers from first wallet... Sleeping for 3 seconds.");
    tokio::time::sleep(Duration::from_secs(3)).await;

    let cash_notes_for_2: Vec<_> = transfer_to_2.cash_notes_for_recipient.clone();
    let cash_notes_for_3: Vec<_> = transfer_to_3.cash_notes_for_recipient.clone();

    let could_err1 = client.verify_cashnote(&cash_notes_for_2[0]).await;
    let could_err2 = client.verify_cashnote(&cash_notes_for_3[0]).await;
    info!("Both should fail during GET record accumulation : {could_err1:?} {could_err2:?}");
    assert!(could_err1.is_err() && could_err2.is_err());

    Ok(())
}

#[tokio::test]
async fn genesis_double_spend_fail() -> Result<()> {
    let _log_guards = LogBuilder::init_single_threaded_tokio_test("double_spend", true);

    // create a client and an unused wallet to make sure some money already exists in the system
    let first_wallet_dir = TempDir::new()?;
    let (client, mut first_wallet) = get_client_and_funded_wallet(first_wallet_dir.path()).await?;
    let first_wallet_addr = first_wallet.address();

    // create a new genesis wallet with the intention to spend genesis again
    let second_wallet_dir = TempDir::new()?;
    let mut second_wallet = HotWallet::create_from_key(&second_wallet_dir, get_genesis_sk())?;
    second_wallet.deposit_and_store_to_disk(&vec![GENESIS_CASHNOTE.clone()])?;
    let genesis_amount = GENESIS_CASHNOTE.value()?;
    let second_wallet_addr = second_wallet.address();

    // create a transfer from the second wallet to the first wallet
    // this will spend Genesis (again) and transfer its value to the first wallet
    let (genesis_cashnote, exclusive_access) = second_wallet.available_cash_notes()?;
    let mut rng = rng::thread_rng();
    let recipient = (
        genesis_amount,
        first_wallet_addr,
        DerivationIndex::random(&mut rng),
    );
    let change_addr = second_wallet_addr;
    let reason = SpendReason::default();
    let transfer = OfflineTransfer::new(genesis_cashnote, vec![recipient], change_addr, reason)?;

    // send the transfer to the network which will mark genesis as a double spent
    // making its direct descendants unspendable
    let res = client
        .send_spends(transfer.all_spend_requests.iter(), false)
        .await;
    std::mem::drop(exclusive_access);
    assert!(res.is_ok());

    // put the bad cashnote in the first wallet
    first_wallet.deposit_and_store_to_disk(&transfer.cash_notes_for_recipient)?;

    // now try to spend this illegitimate cashnote (direct descendant of double spent genesis)
    let (genesis_cashnote_and_others, exclusive_access) = first_wallet.available_cash_notes()?;
    let recipient = (
        genesis_amount,
        second_wallet_addr,
        DerivationIndex::random(&mut rng),
    );
    let bad_genesis_descendant = genesis_cashnote_and_others
        .iter()
        .find(|(cn, _)| cn.value().unwrap() == genesis_amount)
        .unwrap()
        .clone();
    let change_addr = first_wallet_addr;
    let reason = SpendReason::default();
    let transfer2 = OfflineTransfer::new(
        vec![bad_genesis_descendant],
        vec![recipient],
        change_addr,
        reason,
    )?;

    // send the transfer to the network which should reject it
    let res = client
        .send_spends(transfer2.all_spend_requests.iter(), false)
        .await;
    std::mem::drop(exclusive_access);
    assert_matches!(res, Err(WalletError::CouldNotSendMoney(_)));

    Ok(())
}

#[tokio::test]
async fn poisoning_old_spend_should_not_affect_descendant() -> Result<()> {
    let _log_guards = LogBuilder::init_single_threaded_tokio_test("double_spend", true);
    let mut rng = rng::thread_rng();
    let reason = SpendReason::default();
    // create 1 wallet add money from faucet
    let wallet_dir_1 = TempDir::new()?;

    let (client, mut wallet_1) = get_client_and_funded_wallet(wallet_dir_1.path()).await?;
    let balance_1 = wallet_1.balance().as_nano();
    let amount = NanoTokens::from(balance_1 / 2);
    let to1 = wallet_1.address();

    // Send from 1 -> 2
    let wallet_dir_2 = TempDir::new()?;
    let mut wallet_2 = get_wallet(wallet_dir_2.path());
    assert_eq!(wallet_2.balance(), NanoTokens::zero());

    let to2 = wallet_2.address();
    let (cash_notes_1, _exclusive_access) = wallet_1.available_cash_notes()?;
    let to_2_unique_key = (amount, to2, DerivationIndex::random(&mut rng));
    let transfer_to_2 = OfflineTransfer::new(
        cash_notes_1.clone(),
        vec![to_2_unique_key],
        to1,
        reason.clone(),
    )
    .unwrap();

    info!("Sending 1->2 to the network...");
    client
        .send_spends(transfer_to_2.all_spend_requests.iter(), false)
        .await?;
    // std::mem::drop(exclusive_access);

    info!("Verifying the transfers from 1 -> 2 wallet...");
    let cash_notes_for_2: Vec<_> = transfer_to_2.cash_notes_for_recipient.clone();
    client.verify_cashnote(&cash_notes_for_2[0]).await?;
    wallet_2.deposit_and_store_to_disk(&cash_notes_for_2)?; // store inside 2

    // Send from 2 -> 22
    let wallet_dir_22 = TempDir::new()?;
    let mut wallet_22 = get_wallet(wallet_dir_22.path());
    assert_eq!(wallet_22.balance(), NanoTokens::zero());

    let (cash_notes_2, _exclusive_access) = wallet_2.available_cash_notes()?;
    assert!(!cash_notes_2.is_empty());
    let to_22_unique_key = (
        wallet_2.balance(),
        wallet_22.address(),
        DerivationIndex::random(&mut rng),
    );
    let transfer_to_22 =
        OfflineTransfer::new(cash_notes_2, vec![to_22_unique_key], to2, reason.clone()).unwrap();

    client
        .send_spends(transfer_to_22.all_spend_requests.iter(), false)
        .await?;
    // std::mem::drop(exclusive_access);

    info!("Verifying the transfers from 2 -> 22 wallet...");
    let cash_notes_for_22: Vec<_> = transfer_to_22.cash_notes_for_recipient.clone();
    client.verify_cashnote(&cash_notes_for_22[0]).await?;
    wallet_22.deposit_and_store_to_disk(&cash_notes_for_22)?; // store inside 22

    // Try to double spend from 1 -> 3
    let wallet_dir_3 = TempDir::new()?;
    let wallet_3 = get_wallet(wallet_dir_3.path());
    assert_eq!(wallet_3.balance(), NanoTokens::zero());

    let to_3_unique_key = (
        amount,
        wallet_3.address(),
        DerivationIndex::random(&mut rng),
    );
    let transfer_to_3 =
        OfflineTransfer::new(cash_notes_1, vec![to_3_unique_key], to1, reason.clone()).unwrap(); // reuse the old cash notes
    client
        .send_spends(transfer_to_3.all_spend_requests.iter(), false)
        .await?;
    info!("Verifying the transfers from 1 -> 3 wallet... It should error out.");
    let cash_notes_for_3: Vec<_> = transfer_to_3.cash_notes_for_recipient.clone();
    assert!(client.verify_cashnote(&cash_notes_for_3[0]).await.is_err()); // the old spend has been poisoned

    // The old spend has been poisoned, but spends from 22 -> 222 should still work
    let wallet_dir_222 = TempDir::new()?;
    let wallet_222 = get_wallet(wallet_dir_222.path());
    assert_eq!(wallet_222.balance(), NanoTokens::zero());

    let (cash_notes_22, _exclusive_access) = wallet_22.available_cash_notes()?;
    assert!(!cash_notes_22.is_empty());
    let to_222_unique_key = (
        wallet_22.balance(),
        wallet_222.address(),
        DerivationIndex::random(&mut rng),
    );
    let transfer_to_222 = OfflineTransfer::new(
        cash_notes_22,
        vec![to_222_unique_key],
        wallet_22.address(),
        reason,
    )
    .unwrap();
    client
        .send_spends(transfer_to_222.all_spend_requests.iter(), false)
        .await?;
    info!("Verifying the transfers from 22 -> 222 wallet...");
    let cash_notes_for_222: Vec<_> = transfer_to_222.cash_notes_for_recipient.clone();
    client.verify_cashnote(&cash_notes_for_222[0]).await?;

    Ok(())
}
