// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

#![allow(dead_code)]

use self_encryption::MIN_ENCRYPTABLE_BYTES;
use sn_client::{load_faucet_wallet_from_genesis_wallet, send, Client, Files};
use sn_peers_acquisition::parse_peer_addr;
use sn_protocol::safenode_proto::{
    safe_node_client::SafeNodeClient, NodeInfoRequest, RestartRequest,
};
use sn_protocol::storage::ChunkAddress;
use sn_transfers::LocalWallet;

use bytes::Bytes;
use eyre::{eyre, Result};
use lazy_static::lazy_static;
use rand::{
    distributions::{Distribution, Standard},
    Rng,
};
use sn_transfers::NanoTokens;
use std::{
    fs::File,
    io::Write,
    net::SocketAddr,
    path::{Path, PathBuf},
};
use tokio::sync::Mutex;
use tonic::Request;
use xor_name::XorName;

type ResultRandomContent = Result<(Files, Bytes, ChunkAddress, Vec<(XorName, PathBuf)>)>;

pub const PAYING_WALLET_INITIAL_BALANCE: u64 = 100_000_000_000_000;

lazy_static! {
    // mutex to restrict access to faucet wallet from concurrent tests
    static ref FAUCET_WALLET_MUTEX: Mutex<()> = Mutex::new(());
}

///  Get a new Client for testing
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
    Client::new(secret_key, bootstrap_peers)
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
    let wallet_balance = NanoTokens::from(amount);
    let mut local_wallet = get_wallet(root_dir).await;

    println!("Getting {wallet_balance} tokens from the faucet...");
    let tokens = send(from, wallet_balance, local_wallet.address(), client, true).await?;

    println!("Verifying the transfer from faucet...");
    client.verify(&tokens).await?;
    local_wallet.deposit_and_store_to_disk(&vec![tokens])?;
    assert_eq!(local_wallet.balance(), wallet_balance);
    println!("CashNotes deposited to the wallet that'll pay for storage: {wallet_balance}.");

    Ok(local_wallet)
}

pub async fn get_client_and_wallet(root_dir: &Path, amount: u64) -> Result<(Client, LocalWallet)> {
    let _guard = FAUCET_WALLET_MUTEX.lock().await;

    let client = get_client().await;
    let faucet = load_faucet_wallet_from_genesis_wallet(&client).await?;
    let local_wallet = get_funded_wallet(&client, faucet, root_dir, amount).await?;

    Ok((client, local_wallet))
}

pub fn random_content(
    client: &Client,
    wallet_dir: PathBuf,
    chunk_dir: PathBuf,
) -> ResultRandomContent {
    let mut rng = rand::thread_rng();

    let random_len = rng.gen_range(MIN_ENCRYPTABLE_BYTES..1024 * MIN_ENCRYPTABLE_BYTES);
    let random_length_content: Vec<u8> =
        <Standard as Distribution<u8>>::sample_iter(Standard, &mut rng)
            .take(random_len)
            .collect();

    let file_path = chunk_dir.join("random_content");
    let mut output_file = File::create(file_path.clone())?;
    output_file.write_all(&random_length_content)?;

    let files_api = Files::new(client.clone(), wallet_dir);
    let (file_addr, _file_size, chunks) = Files::chunk_file(&file_path, &chunk_dir)?;

    Ok((
        files_api,
        random_length_content.into(),
        ChunkAddress::new(file_addr),
        chunks,
    ))
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
