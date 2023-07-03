// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod data_with_churn;

use std::path::Path;

use sn_client::{get_tokens_from_faucet, send, Client, WalletClient};

use sn_dbc::{random_derivation_index, rng, Token};
use sn_transfers::{client_transfers::create_transfer, wallet::LocalWallet};
use tracing_core::Level;

use assert_fs::TempDir;
use eyre::Result;
use rand::Rng;
use xor_name::XorName;

async fn get_client() -> Client {
    let secret_key = bls::SecretKey::random();
    Client::new(secret_key, None, None)
        .await
        .expect("Client shall be successfully created.")
}

async fn get_wallet(root_dir: &Path) -> LocalWallet {
    LocalWallet::load_from(root_dir)
        .await
        .expect("Wallet shall be successfully created.")
}

#[tokio::test(flavor = "multi_thread")]
async fn multiple_sequential_transfers_succeed() -> Result<()> {
    let logging_targets = vec![
        ("safenode".to_string(), Level::INFO),
        ("sn_client".to_string(), Level::TRACE),
        ("sn_transfers".to_string(), Level::INFO),
        ("sn_networking".to_string(), Level::INFO),
        ("sn_node".to_string(), Level::INFO),
    ];
    let _log_appender_guard = sn_logging::init_logging(logging_targets, &None, false)?;

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

#[tokio::test(flavor = "multi_thread")]
async fn double_spend_transfers_fail() -> Result<()> {
    let logging_targets = vec![
        ("safenode".to_string(), Level::INFO),
        ("sn_client".to_string(), Level::TRACE),
        ("sn_transfers".to_string(), Level::INFO),
        ("sn_networking".to_string(), Level::INFO),
        ("sn_node".to_string(), Level::INFO),
    ];
    let _log_appender_guard = sn_logging::init_logging(logging_targets, &None, false)?;

    // create 1 wallet add money from faucet
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

    // create wallet 2 and 3 to receive money from 1
    let second_wallet_dir = TempDir::new()?;
    let second_wallet = get_wallet(second_wallet_dir.path()).await;
    assert_eq!(second_wallet.balance(), Token::zero());
    let third_wallet_dir = TempDir::new()?;
    let third_wallet = get_wallet(third_wallet_dir.path()).await;
    assert_eq!(third_wallet.balance(), Token::zero());

    // manually forge two transfers of the same source
    let amount = Token::from_nano(first_wallet_balance.as_nano() / 3);
    let to1 = first_wallet.address();
    let to2 = second_wallet.address();
    let to3 = third_wallet.address();

    let some_dbcs = first_wallet.available_dbcs();
    let same_dbcs = some_dbcs.clone();

    let mut rng = rng::thread_rng();

    let to2_unique_key = (amount, to2, random_derivation_index(&mut rng));
    let to3_unique_key = (amount, to3, random_derivation_index(&mut rng));
    let reason_hash: sn_dbc::Hash = None.unwrap_or_default();

    let transfer_to_2 = create_transfer(some_dbcs, vec![to2_unique_key], to1, reason_hash).unwrap();
    let transfer_to_3 = create_transfer(same_dbcs, vec![to3_unique_key], to1, reason_hash).unwrap();

    // send both transfers to the network
    println!("Sending both transfers to the network...");
    let res = client.send(transfer_to_2.clone()).await;
    assert!(res.is_ok());
    let res = client.send(transfer_to_3.clone()).await;
    assert!(res.is_err());

    // check the DBCs, it should fail
    std::thread::sleep(std::time::Duration::from_secs(5));
    println!("Verifying the transfers from first wallet...");
    let dbcs_for_2: Vec<_> = transfer_to_2
        .created_dbcs
        .iter()
        .map(|d| d.dbc.clone())
        .collect();
    let dbcs_for_3: Vec<_> = transfer_to_3
        .created_dbcs
        .iter()
        .map(|d| d.dbc.clone())
        .collect();
    let should_err1 = client.verify(&dbcs_for_2[0]).await;
    let should_err2 = client.verify(&dbcs_for_3[0]).await;
    println!("Verifying at least one fails: {should_err1:?} {should_err2:?}");
    assert!(should_err1.is_err() || should_err2.is_err());

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn storage_payment_succeeds() -> Result<()> {
    let logging_targets = vec![
        ("safenode".to_string(), Level::INFO),
        ("sn_client".to_string(), Level::TRACE),
        ("sn_transfers".to_string(), Level::INFO),
        ("sn_networking".to_string(), Level::INFO),
        ("sn_node".to_string(), Level::INFO),
    ];
    let _log_appender_guard = sn_logging::init_logging(logging_targets, &None, false)?;

    let paying_wallet_dir = TempDir::new()?;
    let paying_wallet_balance = Token::from_nano(500_000);

    let mut paying_wallet = get_wallet(paying_wallet_dir.path()).await;
    let client = get_client().await;
    println!("Getting {paying_wallet_balance} tokens from the faucet...");
    let tokens =
        get_tokens_from_faucet(paying_wallet_balance, paying_wallet.address(), &client).await;
    std::thread::sleep(std::time::Duration::from_secs(5));
    println!("Verifying the transfer from faucet...");
    client.verify(&tokens).await?;
    paying_wallet.deposit(vec![tokens]);
    assert_eq!(paying_wallet.balance(), paying_wallet_balance);
    println!("Tokens deposited to the wallet that'll pay for storage: {paying_wallet_balance}.");

    let mut wallet_client = WalletClient::new(client.clone(), paying_wallet);

    // generate a random number (between 50 and 100) of random addresses
    let mut rng = rand::thread_rng();
    let random_content_addrs = (0..rng.gen_range(50..100))
        .collect::<Vec<_>>()
        .iter()
        .map(|_| XorName::random(&mut rng))
        .collect::<Vec<_>>();
    println!(
        "Paying for {} random addresses...",
        random_content_addrs.len()
    );

    let proofs = wallet_client
        .pay_for_storage(random_content_addrs.iter())
        .await?;

    std::thread::sleep(std::time::Duration::from_secs(5));

    let cost = proofs.len() as u64; // 1 nano per addr
    let new_balance = Token::from_nano(paying_wallet_balance.as_nano() - cost);
    println!("Verifying new balance on paying wallet is {new_balance} ...");
    let paying_wallet = wallet_client.into_wallet();
    assert_eq!(paying_wallet.balance(), new_balance);

    Ok(())
}
