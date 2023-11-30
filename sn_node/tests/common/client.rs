// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use eyre::Result;
use futures::future::join_all;
use lazy_static::lazy_static;
use sn_client::{load_faucet_wallet_from_genesis_wallet, send, Client};
use sn_peers_acquisition::parse_peer_addr;
use sn_protocol::test_utils::DeploymentInventory;
use sn_transfers::{LocalWallet, NanoTokens};
use std::path::Path;
use tokio::sync::Mutex;
use tracing::error;

pub const PAYING_WALLET_INITIAL_BALANCE: u64 = 100_000_000_000_000;

lazy_static! {
    // mutex to restrict access to faucet wallet from concurrent tests
    static ref FAUCET_WALLET_MUTEX: Mutex<()> = Mutex::new(());
}

/// Load LocalWallet from dir
pub async fn get_wallet(root_dir: &Path) -> LocalWallet {
    LocalWallet::load_from(root_dir).expect("Wallet shall be successfully created.")
}

/// Get a new Client.
/// If SN_INVENTORY flag is passed, the client is bootstrapped to the droplet network
/// Else to the local network.
pub async fn get_gossip_client() -> Client {
    match DeploymentInventory::load() {
        Ok(inventory) => Droplet::get_gossip_client(inventory.peers).await,
        Err(_) => NonDroplet::get_gossip_client().await,
    }
}

/// Get a funded wallet.
/// If SN_INVENTORY flag is passed, the amount is retrieved from the faucet url
/// else obtain it from the provided `from` LocalWallet
pub async fn get_funded_wallet(
    client: &Client,
    from: LocalWallet,
    root_dir: &Path,
    amount: u64,
) -> Result<LocalWallet> {
    match DeploymentInventory::load() {
        Ok(inventory) => {
            Droplet::get_funded_wallet(root_dir, amount, inventory.faucet_address).await
        }
        Err(_) => NonDroplet::get_funded_wallet(client, from, root_dir, amount).await,
    }
}

/// Create a client and fund the wallet.
/// If SN_INVENTORY flag is passed, the wallet is funded by fetching it from the faucet
/// Else create a genesis wallet and transfer funds from there.
pub async fn get_gossip_client_and_wallet(
    root_dir: &Path,
    amount: u64,
) -> Result<(Client, LocalWallet)> {
    match DeploymentInventory::load() {
        Ok(inventory) => {
            let client = Droplet::get_gossip_client(inventory.peers).await;
            let local_wallet =
                Droplet::get_funded_wallet(root_dir, amount, inventory.faucet_address).await?;
            Ok((client, local_wallet))
        }
        Err(_) => {
            let _guard = FAUCET_WALLET_MUTEX.lock().await;

            let client = NonDroplet::get_gossip_client().await;
            let faucet = load_faucet_wallet_from_genesis_wallet(&client).await?;
            let local_wallet =
                NonDroplet::get_funded_wallet(&client, faucet, root_dir, amount).await?;

            Ok((client, local_wallet))
        }
    }
}

pub struct NonDroplet;
impl NonDroplet {
    ///  Get a new Client for testing
    pub async fn get_gossip_client() -> Client {
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
        Client::new(secret_key, bootstrap_peers, true, None)
            .await
            .expect("Client shall be successfully created.")
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
        client.verify_cashnote(&tokens).await?;
        local_wallet.deposit_and_store_to_disk(&vec![tokens])?;
        assert_eq!(local_wallet.balance(), wallet_balance);
        println!("CashNotes deposited to the wallet that'll pay for storage: {wallet_balance}.");

        Ok(local_wallet)
    }
}

struct Droplet;
impl Droplet {
    /// Create a new client and bootstrap from the provided safe_peers
    pub async fn get_gossip_client(safe_peers: Vec<String>) -> Client {
        let secret_key = bls::SecretKey::random();

        let mut bootstrap_peers = Vec::new();
        for peer in safe_peers {
            match parse_peer_addr(&peer) {
                Ok(peer) => bootstrap_peers.push(peer),
                Err(err) => error!("Can't parse SAFE_PEERS {peer:?} with error {err:?}"),
            }
        }
        if bootstrap_peers.is_empty() {
            panic!("Could parse/find any bootstrap peers");
        }

        println!("Client bootstrap with peer {bootstrap_peers:?}");
        Client::new(secret_key, Some(bootstrap_peers), true, None)
            .await
            .expect("Client shall be successfully created.")
    }

    // Create a wallet at root_dir and fetch the amount from the faucet url
    pub async fn get_funded_wallet(
        root_dir: &Path,
        amount: u64,
        faucet_url: String,
    ) -> Result<LocalWallet> {
        let _guard = FAUCET_WALLET_MUTEX.lock().await;

        let faucet_balance = 100 * 1_000_000_000; // Each request gives 100 SNT
        let num_requests = std::cmp::min((amount + faucet_balance - 1) / faucet_balance, 1);
        let num_requests = std::cmp::max(num_requests, 100); // max 100 req

        let mut local_wallet = get_wallet(root_dir).await;

        println!(
            "Getting {} tokens from the faucet...",
            NanoTokens::from(num_requests * 100 * 1_000_000_000)
        );
        let mut tasks = Vec::new();
        for _ in 0..num_requests {
            let faucet_url = faucet_url.clone();
            let task = tokio::spawn(async move {
                // Get cash_note from faucet
                let cash_note = reqwest::get(&faucet_url).await?.text().await?;
                let cash_note = sn_transfers::CashNote::from_hex(cash_note.trim())?;
                Ok::<_, eyre::Report>(cash_note)
            });
            tasks.push(task);
        }

        for result in join_all(tasks).await {
            let cash_note = result??;
            local_wallet.deposit_and_store_to_disk(&vec![cash_note])?;
        }

        Ok(local_wallet)
    }
}
