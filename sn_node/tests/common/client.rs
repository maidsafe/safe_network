// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use eyre::Result;
use lazy_static::lazy_static;
use sn_client::{load_faucet_wallet_from_genesis_wallet, send, Client};
use sn_peers_acquisition::parse_peer_addr;
use sn_protocol::test_utils::DeploymentInventory;
use sn_transfers::{LocalWallet, NanoTokens, Transfer};
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::Path,
};
use tokio::sync::Mutex;
use tracing::error;

pub const PAYING_WALLET_INITIAL_BALANCE: u64 = 100_000_000_000_000;
/// The node count for a locally running network that the tests expect
pub const LOCAL_NODE_COUNT: usize = 25;

lazy_static! {
    // mutex to restrict access to faucet wallet from concurrent tests
    static ref FAUCET_WALLET_MUTEX: Mutex<()> = Mutex::new(());
}

/// Load LocalWallet from dir
pub fn get_wallet(root_dir: &Path) -> LocalWallet {
    LocalWallet::load_from(root_dir).expect("Wallet shall be successfully created.")
}

/// Get the node count
/// If SN_INVENTORY flag is passed, the node count is obtained from the the droplet
/// else return the local node count
pub fn get_node_count() -> usize {
    match DeploymentInventory::load() {
        Ok(inventory) => inventory.rpc_endpoints.len(),
        Err(_) => LOCAL_NODE_COUNT,
    }
}

/// Get the list of all RPC addresses
/// If SN_INVENTORY flag is passed, the RPC addresses of all the droplet nodes are returned
/// else generate local addresses for NODE_COUNT nodes
pub fn get_all_rpc_addresses() -> Vec<SocketAddr> {
    match DeploymentInventory::load() {
        Ok(inventory) => inventory.rpc_endpoints,
        Err(_) => {
            let mut addresses = Vec::new();
            for i in 1..LOCAL_NODE_COUNT + 1 {
                let addr =
                    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12000 + i as u16);
                addresses.push(addr);
            }
            addresses
        }
    }
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
            Droplet::get_funded_wallet(client, root_dir, amount, inventory.faucet_address).await
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
                Droplet::get_funded_wallet(&client, root_dir, amount, inventory.faucet_address)
                    .await?;
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
        let mut local_wallet = get_wallet(root_dir);

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
        client: &Client,
        root_dir: &Path,
        amount: u64,
        faucet_socket: String,
    ) -> Result<LocalWallet> {
        let _guard = FAUCET_WALLET_MUTEX.lock().await;

        let faucet_balance = 100 * 1_000_000_000; // Each request gives 100 SNT
        let num_requests = std::cmp::max((amount + faucet_balance - 1) / faucet_balance, 1);
        let num_requests = std::cmp::min(num_requests, 10); // max 10 req

        let mut local_wallet = get_wallet(root_dir);
        let address_hex = hex::encode(local_wallet.address().to_bytes());

        println!(
            "Getting {} tokens from the faucet... num_requests:{num_requests}",
            NanoTokens::from(num_requests * 100 * 1_000_000_000)
        );
        for _ in 0..num_requests {
            let faucet_url = format!("http://{faucet_socket}/{address_hex}");

            // Get transfer from faucet
            let transfer = reqwest::get(&faucet_url).await?.text().await?;
            let transfer = match Transfer::from_hex(&transfer) {
                Ok(transfer) => transfer,
                Err(err) => {
                    println!("Failed to parse transfer: {err:?}");
                    println!("Transfer: \"{transfer}\"");
                    return Err(err.into());
                }
            };
            let cashnotes = match client.receive(&transfer, &local_wallet).await {
                Ok(cashnotes) => cashnotes,
                Err(err) => {
                    println!("Failed to verify and redeem transfer: {err:?}");
                    return Err(err.into());
                }
            };
            println!("Successfully verified transfer.");
            local_wallet.deposit_and_store_to_disk(&cashnotes)?;
        }

        Ok(local_wallet)
    }
}
