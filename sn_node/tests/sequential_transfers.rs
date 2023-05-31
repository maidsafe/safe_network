// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod data_with_churn;

use std::path::Path;

use sn_client::{get_tokens_from_faucet, send, Client};

use sn_dbc::Token;
use sn_transfers::wallet::{DepositWallet, LocalWallet, VerifyingClient, Wallet};
use tracing_core::Level;

use assert_fs::TempDir;
use eyre::Result;

#[tokio::test(flavor = "multi_thread")]
#[ignore = "This test is ignored because it is not stable until we have DBCs stored as records."]
async fn multiple_sequential_transfers_succeed() -> Result<()> {
    let logging_targets = vec![
        ("safenode".to_string(), Level::INFO),
        ("sn_transfers".to_string(), Level::INFO),
        ("sn_networking".to_string(), Level::INFO),
        ("sn_node".to_string(), Level::INFO),
    ];
    let _log_appender_guard = sn_logging::init_logging(logging_targets, &None)?;

    let first_wallet_dir = TempDir::new()?;
    let first_wallet_balance = Token::from_nano(1_000_000_000);

    let mut first_wallet = get_wallet(first_wallet_dir.path()).await;
    let client = get_client().await;
    println!("Getting {first_wallet_balance} tokens from the faucet...");
    let tokens =
        get_tokens_from_faucet(first_wallet_balance, first_wallet.address(), &client).await;
    std::thread::sleep(std::time::Duration::from_secs(5));
    println!("Verifying the transfer from faucet...");
    client.verify(&tokens).await?;
    first_wallet.deposit(vec![tokens]);
    assert_eq!(first_wallet.balance(), first_wallet_balance);
    println!("Tokens deposited to first wallet: {first_wallet_balance}.");

    let second_wallet_balance = Token::from_nano(first_wallet_balance.as_nano() / 2);
    println!("Transferring from first wallet to second wallet: {second_wallet_balance}.");
    let second_wallet_dir = TempDir::new()?;
    let mut second_wallet = get_wallet(second_wallet_dir.path()).await;

    assert_eq!(second_wallet.balance(), Token::zero());

    let tokens = send(
        first_wallet,
        second_wallet_balance,
        second_wallet.address(),
        &client,
    )
    .await;
    std::thread::sleep(std::time::Duration::from_secs(5));
    println!("Verifying the transfer from first wallet...");
    client.verify(&tokens).await?;
    second_wallet.deposit(vec![tokens]);
    assert_eq!(second_wallet.balance(), second_wallet_balance);
    println!("Tokens deposited to second wallet: {second_wallet_balance}.");

    let first_wallet = get_wallet(first_wallet_dir.path()).await;
    assert!(second_wallet_balance.as_nano() == first_wallet.balance().as_nano());

    Ok(())
}

async fn get_client() -> Client {
    let secret_key = bls::SecretKey::random();
    Client::new(secret_key, None)
        .await
        .expect("Client shall be successfully created.")
}

async fn get_wallet(root_dir: &Path) -> LocalWallet {
    LocalWallet::load_from(root_dir)
        .await
        .expect("Wallet shall be successfully created.")
}
