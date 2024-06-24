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
use eyre::Result;
use sn_client::send;
use sn_logging::LogBuilder;
use sn_transfers::NanoTokens;
use tracing::info;

#[tokio::test]
async fn cash_note_transfer_multiple_sequential_succeed() -> Result<()> {
    let _log_guards = LogBuilder::init_single_threaded_tokio_test("sequential_transfer", true);

    let first_wallet_dir = TempDir::new()?;

    let (client, first_wallet) = get_client_and_funded_wallet(first_wallet_dir.path()).await?;
    let first_wallet_balance = first_wallet.balance().as_nano();

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
