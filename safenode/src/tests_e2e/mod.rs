// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use std::{path::Path, time::Duration};

use crate::{
    client::Client,
    domain::{
        dbc_genesis::{get_tokens_from_faucet, send},
        wallet::{DepositWallet, LocalWallet, VerifyingClient, Wallet},
    },
};

use sn_dbc::Token;

use assert_fs::TempDir;
use eyre::Result;
use std::process::{Command, Stdio};
use tokio::time::sleep;

#[ignore = "Manual use only."]
#[tokio::test(flavor = "multi_thread")]
async fn upload_churn_download() {
    start_network();
    sleep(Duration::from_secs(10)).await;
    upload_files();
    sleep(Duration::from_secs(10)).await;
    churn().await;
    sleep(Duration::from_secs(10)).await;
    download_files();
}

#[ignore = "Not yet finished."]
#[tokio::test(flavor = "multi_thread")]
async fn multiple_sequential_transfers_succeed() -> Result<()> {
    // let _log_appender_guard = crate::log::init_node_logging(&None)?;

    let first_wallet_dir = TempDir::new()?;
    let first_wallet_balance = Token::from_nano(1_000_000_000);

    let mut first_wallet = get_wallet(first_wallet_dir.path()).await;
    let client = get_client();
    println!("Getting {first_wallet_balance} tokens from the faucet...");
    let tokens =
        get_tokens_from_faucet(first_wallet_balance, first_wallet.address(), &client).await;
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
    println!("Verifying the transfer from first wallet...");
    client.verify(&tokens).await?;
    second_wallet.deposit(vec![tokens]);
    assert_eq!(second_wallet.balance(), second_wallet_balance);
    println!("Tokens deposited to second wallet: {second_wallet_balance}.");

    // The first wallet will have paid fees for the transfer,
    // so it will have less than half the amount left, but we can't
    // know how much exactly, so we just check that it has less than
    // the original amount.
    let first_wallet = get_wallet(first_wallet_dir.path()).await;
    assert!(second_wallet_balance.as_nano() > first_wallet.balance().as_nano());

    Ok(())
}

fn get_client() -> Client {
    let secret_key = bls::SecretKey::random();
    Client::new(secret_key).expect("Client shall be successfully created.")
}

async fn get_wallet(root_dir: &Path) -> LocalWallet {
    LocalWallet::load_from(root_dir)
        .await
        .expect("Wallet shall be successfully created.")
}

// Start a network.
fn start_network() {
    let node_bin_path = Path::new("../target/release/testnet");
    let args = vec![
        "--interval",
        "1",
        "--node-path",
        "../target/release/safenode",
    ];
    let _ = Command::new(node_bin_path)
        .args(args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("Cmd should run successfully.");
}

// Upload files.
fn upload_files() {
    let node_bin_path = Path::new("../target/release/safe");
    let args = vec!["files", "upload", "--", "../resources"];
    let _ = Command::new(node_bin_path)
        .args(args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("Cmd should run successfully.");
}

// Download files.
fn download_files() {
    let node_bin_path = Path::new("../target/release/safe");
    let args = vec!["files", "download"];
    let _ = Command::new(node_bin_path)
        .args(args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("Cmd should run successfully.");
}

async fn churn() {
    let base_port = 12000;
    let rpc_path = Path::new("../target/release/examples/safenode_rpc_client");
    for i in 1..26 {
        let address = format!("127.0.0.1:{}", base_port + i);
        let args = vec![address.as_str(), "stop", "1"];
        let _ = Command::new(rpc_path)
            .args(args)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("safenode_rpc_client cmd should run successfully.");

        let node_bin_path = Path::new("../target/release/testnet");
        let args = vec![
            "--join",
            "--node-count",
            "1",
            "--node-path",
            "../target/release/safenode",
        ];
        let _ = Command::new(node_bin_path)
            .args(args)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("Cmd should run successfully.");

        sleep(Duration::from_secs(5)).await;
    }
}
