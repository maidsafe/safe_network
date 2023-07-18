// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use sn_client::{get_tokens_from_faucet, Client};
use sn_peers_acquisition::parse_peer_addr;
use sn_transfers::wallet::LocalWallet;

use eyre::Result;
use lazy_static::lazy_static;
use sn_dbc::Token;
use sn_logging::{LogFormat, LogOutputDest};
use std::{path::Path, sync::Once};
use tokio::sync::Mutex;
use tracing_core::Level;

static TEST_INIT_LOGGER: Once = Once::new();

#[allow(dead_code)]
pub fn init_logging() {
    TEST_INIT_LOGGER.call_once(|| {
        let logging_targets = vec![
            ("safenode".to_string(), Level::INFO),
            ("sn_client".to_string(), Level::TRACE),
            ("sn_transfers".to_string(), Level::INFO),
            ("sn_networking".to_string(), Level::INFO),
            ("sn_node".to_string(), Level::INFO),
        ];
        let _log_appender_guard =
            sn_logging::init_logging(logging_targets, LogOutputDest::Stdout, LogFormat::Default)
                .expect("Failed to init logging");
    });
}

lazy_static! {
    // mutex to restrict access to faucet wallet from concurrent tests
    static ref FAUCET_WALLET_MUTEX: Mutex<()> = Mutex::new(());
}

pub async fn get_client() -> Client {
    let secret_key = bls::SecretKey::random();

    let bootstrap_peers = if !cfg!(feature = "local-discovery") {
        match std::env::var("SAFE_PEERS") {
            Ok(str) => match parse_peer_addr(&str) {
                Ok(peer) => Some(vec![peer]),
                Err(err) => panic!("Can't parse SAFE_PEERS {str:?} with error {err:?}"),
            },
            Err(err) => panic!("Can't get env var SAFE_PEERS with error {err:?}"),
        }
    } else {
        None
    };
    println!("Client bootstrap with peer {bootstrap_peers:?}");
    Client::new(secret_key, bootstrap_peers, None)
        .await
        .expect("Client shall be successfully created.")
}

pub async fn get_wallet(root_dir: &Path) -> LocalWallet {
    LocalWallet::load_from(root_dir)
        .await
        .expect("Wallet shall be successfully created.")
}

pub async fn get_client_and_wallet(root_dir: &Path, amount: u64) -> Result<(Client, LocalWallet)> {
    let _guard = FAUCET_WALLET_MUTEX.lock().await;

    let wallet_balance = Token::from_nano(amount);
    let mut local_wallet = get_wallet(root_dir).await;
    let client = get_client().await;

    println!("Getting {wallet_balance} tokens from the faucet...");
    let tokens = get_tokens_from_faucet(wallet_balance, local_wallet.address(), &client).await;
    std::thread::sleep(std::time::Duration::from_secs(5));

    println!("Verifying the transfer from faucet...");
    client.verify(&tokens).await?;
    local_wallet.deposit(vec![tokens]);
    assert_eq!(local_wallet.balance(), wallet_balance);
    println!("Tokens deposited to the wallet that'll pay for storage: {wallet_balance}.");

    Ok((client, local_wallet))
}
