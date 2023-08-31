// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

#![allow(dead_code)]

#[allow(unused_qualifications, unreachable_pub, clippy::unwrap_used)]
pub mod safenode_proto {
    tonic::include_proto!("safenode_proto");
}

use safenode_proto::{safe_node_client::SafeNodeClient, NodeInfoRequest, RestartRequest};
use sn_client::{load_faucet_wallet_from_genesis_wallet, send, Client};
use sn_peers_acquisition::parse_peer_addr;
use sn_transfers::wallet::LocalWallet;

use eyre::{eyre, Result};
use lazy_static::lazy_static;
use sn_dbc::Token;
use sn_logging::{LogFormat, LogOutputDest};
use std::{net::SocketAddr, path::Path, sync::Once};
use tokio::sync::Mutex;
use tonic::Request;
use tracing_core::Level;

static TEST_INIT_LOGGER: Once = Once::new();

pub const PAYING_WALLET_INITIAL_BALANCE: u64 = 100_000_000_000_000;

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

//  Get a new Client for testing
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
    Client::new(secret_key, bootstrap_peers, None, None)
        .await
        .expect("Client shall be successfully created.")
}

pub async fn get_wallet(root_dir: &Path) -> LocalWallet {
    LocalWallet::load_from(root_dir).expect("Wallet shall be successfully created.")
}

pub async fn get_funded_wallet(
    client: &Client,
    from: LocalWallet,
    root_dir: &Path,
    amount: u64,
) -> Result<LocalWallet> {
    let wallet_balance = Token::from_nano(amount);
    let mut local_wallet = get_wallet(root_dir).await;

    println!("Getting {wallet_balance} tokens from the faucet...");
    let tokens = send(from, wallet_balance, local_wallet.address(), client, true).await?;

    println!("Verifying the transfer from faucet...");
    client.verify(&tokens).await?;
    local_wallet.deposit(&vec![tokens])?;
    assert_eq!(local_wallet.balance(), wallet_balance);
    println!("Tokens deposited to the wallet that'll pay for storage: {wallet_balance}.");

    Ok(local_wallet)
}

pub async fn get_client_and_wallet(root_dir: &Path, amount: u64) -> Result<(Client, LocalWallet)> {
    let _guard = FAUCET_WALLET_MUTEX.lock().await;

    let client = get_client().await;
    let faucet = load_faucet_wallet_from_genesis_wallet(&client).await?;
    let local_wallet = get_funded_wallet(&client, faucet, root_dir, amount).await?;

    Ok((client, local_wallet))
}

pub async fn node_restart(addr: SocketAddr) -> Result<()> {
    let endpoint = format!("https://{addr}");
    let mut client = SafeNodeClient::connect(endpoint).await?;

    let response = client.node_info(Request::new(NodeInfoRequest {})).await?;
    let log_dir = Path::new(&response.get_ref().log_dir);
    let root_dir = log_dir
        .parent()
        .ok_or_else(|| eyre!("could not obtain parent from logging directory"))?;

    // remove Chunks records
    let chunks_records = root_dir.join("record_store");
    if let Ok(true) = chunks_records.try_exists() {
        println!("Removing Chunks records from {}", chunks_records.display());
        std::fs::remove_dir_all(chunks_records)?;
    }

    // remove Registers records
    let registers_records = root_dir.join("registers");
    if let Ok(true) = registers_records.try_exists() {
        println!(
            "Removing Registers records from {}",
            registers_records.display()
        );
        std::fs::remove_dir_all(registers_records)?;
    }

    let _response = client
        .restart(Request::new(RestartRequest { delay_millis: 0 }))
        .await?;

    println!(
        "Node restart requested to RPC service at {addr}, and removed all its chunks and registers records at {}",
        log_dir.display()
    );

    Ok(())
}
