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
    rng, DerivationIndex, Hash, HotWallet, MainSecretKey, NanoTokens, OfflineTransfer, WalletError,
    GENESIS_CASHNOTE, GENESIS_CASHNOTE_SK,
};
use tracing::info;

#[tokio::test]
async fn cash_note_transfer_double_spend_fail() -> Result<()> {
    let _log_guards =
        LogBuilder::init_single_threaded_tokio_test("cash_note_transfer_double_spend");

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

    let to2_unique_key = (amount, to2, DerivationIndex::random(&mut rng));
    let to3_unique_key = (amount, to3, DerivationIndex::random(&mut rng));
    let reason_hash = Hash::default();

    let transfer_to_2 =
        OfflineTransfer::new(some_cash_notes, vec![to2_unique_key], to1, reason_hash).unwrap();
    let transfer_to_3 =
        OfflineTransfer::new(same_cash_notes, vec![to3_unique_key], to1, reason_hash).unwrap();

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

    let cash_notes_for_2: Vec<_> = transfer_to_2.cash_notes_for_recipient.clone();
    let cash_notes_for_3: Vec<_> = transfer_to_3.cash_notes_for_recipient.clone();

    let could_err1 = client.verify_cashnote(&cash_notes_for_2[0]).await;
    let could_err2 = client.verify_cashnote(&cash_notes_for_3[0]).await;
    info!("Verifying at least one fails : {could_err1:?} {could_err2:?}");
    assert!(could_err1.is_err() || could_err2.is_err());

    Ok(())
}

#[tokio::test]
async fn genesis_double_spend_fail() -> Result<()> {
    let _log_guards = LogBuilder::init_single_threaded_tokio_test("genesis_double_spend");

    // create a client and an unused wallet to make sure some money already exists in the system
    let first_wallet_dir = TempDir::new()?;
    let (client, mut first_wallet) = get_client_and_funded_wallet(first_wallet_dir.path()).await?;
    let first_wallet_addr = first_wallet.address();

    // create a new genesis wallet with the intention to spend genesis again
    let second_wallet_dir = TempDir::new()?;
    let secret_key = bls::SecretKey::from_hex(GENESIS_CASHNOTE_SK)?;
    let main_key = MainSecretKey::new(secret_key);
    let mut second_wallet = HotWallet::create_from_key(&second_wallet_dir, main_key)?;
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
    let reason_hash = Hash::default();
    let transfer =
        OfflineTransfer::new(genesis_cashnote, vec![recipient], change_addr, reason_hash)?;

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
    let reason_hash = Hash::default();
    let transfer2 = OfflineTransfer::new(
        vec![bad_genesis_descendant],
        vec![recipient],
        change_addr,
        reason_hash,
    )?;

    // send the transfer to the network which should reject it
    let res = client
        .send_spends(transfer2.all_spend_requests.iter(), false)
        .await;
    std::mem::drop(exclusive_access);
    assert_matches!(res, Err(WalletError::CouldNotSendMoney(_)));

    Ok(())
}
