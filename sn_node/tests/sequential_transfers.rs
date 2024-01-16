// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod common;

use assert_fs::TempDir;
use common::client::{get_gossip_client_and_wallet, get_wallet};
use eyre::Result;
use sn_client::send;
use sn_logging::LogBuilder;
use sn_transfers::{create_offline_transfer, rng, DerivationIndex, Hash, NanoTokens};
use tracing::info;

#[tokio::test]
async fn cash_note_transfer_multiple_sequential_succeed() -> Result<()> {
    let _log_guards = LogBuilder::init_single_threaded_tokio_test("sequential_transfer");

    let first_wallet_balance = 1_000_000_000;
    let first_wallet_dir = TempDir::new()?;

    let (client, first_wallet) =
        get_gossip_client_and_wallet(first_wallet_dir.path(), first_wallet_balance).await?;

    let second_wallet_balance = NanoTokens::from(first_wallet_balance / 2);
    info!("Transferring from first wallet to second wallet: {second_wallet_balance}.");
    let second_wallet_dir = TempDir::new()?;
    let mut second_wallet = get_wallet(second_wallet_dir.path());

    assert_eq!(second_wallet.balance(), NanoTokens::zero());

    let tokens = send(
        first_wallet,
        second_wallet_balance,
        second_wallet.address(),
        &client,
        true,
    )
    .await?;
    info!("Verifying the transfer from first wallet...");

    client.verify_cashnote(&tokens).await?;
    second_wallet.deposit_and_store_to_disk(&vec![tokens])?;
    assert_eq!(second_wallet.balance(), second_wallet_balance);
    info!("CashNotes deposited to second wallet: {second_wallet_balance}.");

    let first_wallet = get_wallet(&first_wallet_dir);
    assert!(second_wallet_balance.as_nano() == first_wallet.balance().as_nano());

    Ok(())
}

#[tokio::test]
async fn cash_note_transfer_double_spend_fail() -> Result<()> {
    let _log_guards = LogBuilder::init_single_threaded_tokio_test("sequential_transfer");

    // create 1 wallet add money from faucet
    let first_wallet_balance = 1_000_000_000;
    let first_wallet_dir = TempDir::new()?;

    let (client, mut first_wallet) =
        get_gossip_client_and_wallet(first_wallet_dir.path(), first_wallet_balance).await?;

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

    let to2_unique_key = (amount, to2, DerivationIndex::random(&mut rng));
    let to3_unique_key = (amount, to3, DerivationIndex::random(&mut rng));
    let reason_hash = Hash::default();

    let transfer_to_2 =
        create_offline_transfer(some_cash_notes, vec![to2_unique_key], to1, reason_hash).unwrap();
    let transfer_to_3 =
        create_offline_transfer(same_cash_notes, vec![to3_unique_key], to1, reason_hash).unwrap();

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
    info!("Verifying the transfers from first wallet...");

    let cash_notes_for_2: Vec<_> = transfer_to_2.created_cash_notes.clone();
    let cash_notes_for_3: Vec<_> = transfer_to_3.created_cash_notes.clone();

    let could_err1 = client.verify_cashnote(&cash_notes_for_2[0]).await;
    let could_err2 = client.verify_cashnote(&cash_notes_for_3[0]).await;
    info!("Verifying at least one fails : {could_err1:?} {could_err2:?}");
    assert!(could_err1.is_err() || could_err2.is_err());

    Ok(())
}
