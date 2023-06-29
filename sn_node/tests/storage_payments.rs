// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod common;

use common::get_client_and_wallet;

use sn_client::WalletClient;
use sn_dbc::{Hash, Token};
use sn_logging::{init_logging, LogFormat, LogOutputDest};
use sn_transfers::wallet::Error;

use assert_fs::TempDir;
use eyre::Result;
use rand::Rng;
use tracing_core::Level;
use xor_name::XorName;

#[tokio::test(flavor = "multi_thread")]
async fn storage_payment_succeeds() -> Result<()> {
    let logging_targets = vec![
        ("safenode".to_string(), Level::INFO),
        ("sn_client".to_string(), Level::TRACE),
        ("sn_transfers".to_string(), Level::INFO),
        ("sn_networking".to_string(), Level::INFO),
        ("sn_node".to_string(), Level::INFO),
    ];
    let _log_appender_guard =
        init_logging(logging_targets, LogOutputDest::Stdout, LogFormat::Default)?;

    let paying_wallet_balance = 500_000;
    let paying_wallet_dir = TempDir::new()?;

    let (client, paying_wallet) =
        get_client_and_wallet(paying_wallet_dir.path(), paying_wallet_balance).await?;
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
    let new_balance = Token::from_nano(paying_wallet_balance - cost);
    println!("Verifying new balance on paying wallet is {new_balance} ...");
    let paying_wallet = wallet_client.into_wallet();
    assert_eq!(paying_wallet.balance(), new_balance);

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn storage_payment_fails() -> Result<()> {
    let logging_targets = vec![
        ("safenode".to_string(), Level::INFO),
        ("sn_client".to_string(), Level::TRACE),
        ("sn_transfers".to_string(), Level::INFO),
        ("sn_networking".to_string(), Level::INFO),
        ("sn_node".to_string(), Level::INFO),
    ];
    let _log_appender_guard =
        init_logging(logging_targets, LogOutputDest::Stdout, LogFormat::Default)?;

    let wallet_dir = TempDir::new()?;
    let (client, mut wallet_client) = get_client_and_wallet(wallet_dir.path(), 15_000).await?;

    // generate a random number (between 50 and 100) of random addresses
    let random_num_of_addrs = rand::thread_rng().gen_range(50..100);
    let storage_cost = Token::from_nano(random_num_of_addrs);

    let mut transfer = wallet_client
        .local_send_storage_payment(storage_cost, Hash::default(), None)
        .await?;

    // let's corrupt the generated spend in any way
    let mut invalid_signed_spend = transfer.all_spend_requests[0].signed_spend.clone();
    invalid_signed_spend.spend.spent_tx.fee.token = Token::from_nano(random_num_of_addrs + 1);
    transfer.all_spend_requests[0].signed_spend = invalid_signed_spend;

    let failed_send = client.send(transfer).await;

    assert!(matches!(failed_send, Err(Error::CouldNotSendTokens(_))));

    Ok(())
}
