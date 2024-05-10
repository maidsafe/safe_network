// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    acc_packet::{create_faucet_account_and_wallet, load_account_wallet_or_create_with_mnemonic},
    send, Client, WalletClient,
};
use sn_peers_acquisition::parse_peer_addr;
use sn_protocol::{storage::Chunk, NetworkAddress};
use sn_transfers::{HotWallet, NanoTokens};

use bls::SecretKey;
use bytes::Bytes;
use eyre::{bail, Result};
use lazy_static::lazy_static;
use rand::distributions::{Distribution, Standard};
use std::path::Path;
use tokio::{
    sync::Mutex,
    time::{Duration, Instant},
};
use tracing::{info, warn};

/// 100 SNT is the amount `get_funded_wallet` funds the created wallet with.
pub const AMOUNT_TO_FUND_WALLETS: u64 = 100 * 1_000_000_000;

// The number of times to try to load the faucet wallet
const LOAD_FAUCET_WALLET_RETRIES: usize = 6;

lazy_static! {
    // mutex to restrict access to faucet wallet from concurrent tests
    static ref FAUCET_WALLET_MUTEX: Mutex<()> = Mutex::new(());
}

/// Get a new Client for testing
pub async fn get_new_client(owner_sk: SecretKey) -> Result<Client> {
    let bootstrap_peers = if cfg!(feature = "local-discovery") {
        None
    } else {
        match std::env::var("SAFE_PEERS") {
            Ok(str) => match parse_peer_addr(&str) {
                Ok(peer) => Some(vec![peer]),
                Err(err) => bail!("Can't parse SAFE_PEERS {str:?} with error {err:?}"),
            },
            Err(err) => bail!("Can't get env var SAFE_PEERS with error {err:?}"),
        }
    };

    println!("Client bootstrap with peer {bootstrap_peers:?}");
    let client = Client::new(owner_sk, bootstrap_peers, None, None).await?;
    Ok(client)
}

/// Generate a Chunk with random bytes
pub fn random_file_chunk() -> Chunk {
    let mut rng = rand::thread_rng();
    let random_content: Vec<u8> = <Standard as Distribution<u8>>::sample_iter(Standard, &mut rng)
        .take(100)
        .collect();
    Chunk::new(Bytes::from(random_content))
}

/// Creates and funds a new hot-wallet at the provided path
pub async fn get_funded_wallet(client: &Client, wallet_dir: &Path) -> Result<HotWallet> {
    let wallet_balance = NanoTokens::from(AMOUNT_TO_FUND_WALLETS);
    let _guard = FAUCET_WALLET_MUTEX.lock().await;
    let from_faucet_wallet = load_faucet_wallet().await?;

    let mut local_wallet = load_account_wallet_or_create_with_mnemonic(wallet_dir, None)
        .expect("Wallet shall be successfully created.");

    println!("Getting {wallet_balance} tokens from the faucet...");
    info!("Getting {wallet_balance} tokens from the faucet...");
    let tokens = send(
        from_faucet_wallet,
        wallet_balance,
        local_wallet.address(),
        client,
        true,
    )
    .await?;

    println!("Verifying the transfer from faucet...");
    info!("Verifying the transfer from faucet...");
    client.verify_cashnote(&tokens).await?;
    local_wallet.deposit_and_store_to_disk(&vec![tokens])?;
    assert_eq!(local_wallet.balance(), wallet_balance);
    println!("CashNotes deposited to the wallet that'll pay for storage: {wallet_balance}.");
    info!("CashNotes deposited to the wallet that'll pay for storage: {wallet_balance}.");

    Ok(local_wallet)
}

/// Pay the network for the provided list of storage addresses.
pub async fn pay_for_storage(
    client: &Client,
    wallet_dir: &Path,
    addrs2pay: Vec<NetworkAddress>,
) -> Result<()> {
    let wallet = load_account_wallet_or_create_with_mnemonic(wallet_dir, None)?;

    let mut wallet_client = WalletClient::new(client.clone(), wallet);
    let _ = wallet_client.pay_for_storage(addrs2pay.into_iter()).await?;
    Ok(())
}

async fn load_faucet_wallet() -> Result<HotWallet> {
    info!("Loading faucet wallet...");
    let now = Instant::now();
    for attempt in 1..LOAD_FAUCET_WALLET_RETRIES + 1 {
        let faucet_wallet = create_faucet_account_and_wallet();

        let faucet_balance = faucet_wallet.balance();
        if !faucet_balance.is_zero() {
            info!("Loaded faucet wallet after {:?}", now.elapsed());
            return Ok(faucet_wallet);
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
        warn!("The faucet wallet is empty. Attempts: {attempt}/{LOAD_FAUCET_WALLET_RETRIES}")
    }
    bail!("The faucet wallet is empty even after {LOAD_FAUCET_WALLET_RETRIES} retries. Bailing after {:?}. Check the faucet_server logs.", now.elapsed());
}
