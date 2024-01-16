// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use eyre::{bail, OptionExt, Result};
use lazy_static::lazy_static;
use sn_client::{send, Client};
use sn_peers_acquisition::parse_peer_addr;
use sn_protocol::node_registry::{get_local_node_registry_path, NodeRegistry};
use sn_protocol::test_utils::DeploymentInventory;
use sn_transfers::{create_faucet_wallet, LocalWallet, NanoTokens, Transfer};
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::Path,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;
use tracing::{error, info, warn};

pub const PAYING_WALLET_INITIAL_BALANCE: u64 = 100_000_000_000_000;
/// The node count for a locally running network that the tests expect
pub const LOCAL_NODE_COUNT: usize = 25;
// The number of times to try to load the faucet wallet
const LOAD_FAUCET_WALLET_RETRIES: usize = 6;

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
///
/// The genesis address is skipped for droplets as we don't want to restart the Genesis node there.
/// The restarted node relies on the genesis multiaddr to bootstrap after restart.
pub fn get_all_rpc_addresses(skip_genesis_for_droplet: bool) -> Result<Vec<SocketAddr>> {
    match DeploymentInventory::load() {
        Ok(inventory) => {
            if !skip_genesis_for_droplet {
                return Ok(inventory.rpc_endpoints.clone());
            }
            // else filter out genesis
            let genesis_ip = inventory
                .vm_list
                .iter()
                .find_map(|(name, addr)| {
                    if name.contains("genesis") {
                        Some(*addr)
                    } else {
                        None
                    }
                })
                .ok_or_eyre("Could not get the genesis VM's addr")?;

            let addrs = inventory
                .rpc_endpoints
                .into_iter()
                .filter(|addr| addr.ip() != genesis_ip)
                .collect();
            Ok(addrs)
        }
        Err(_) => {
            let local_node_reg_path = &get_local_node_registry_path()?;
            let local_node_registry = NodeRegistry::load(local_node_reg_path)?;
            let addresses = local_node_registry
                .nodes
                .iter()
                .map(|n| SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), n.rpc_port))
                .collect::<Vec<SocketAddr>>();
            Ok(addresses)
        }
    }
}

/// Get a new Client.
/// If SN_INVENTORY flag is passed, the client is bootstrapped to the droplet network
/// Else to the local network.
pub async fn get_gossip_client() -> Client {
    match DeploymentInventory::load() {
        Ok(inventory) => Droplet::get_gossip_client(&inventory).await,
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
            let client = Droplet::get_gossip_client(&inventory).await;
            let local_wallet =
                Droplet::get_funded_wallet(&client, root_dir, amount, inventory.faucet_address)
                    .await?;
            Ok((client, local_wallet))
        }
        Err(_) => {
            let _guard = FAUCET_WALLET_MUTEX.lock().await;

            let client = NonDroplet::get_gossip_client().await;

            let faucet_wallet = NonDroplet::load_faucet_wallet().await?;
            let local_wallet =
                NonDroplet::get_funded_wallet(&client, faucet_wallet, root_dir, amount).await?;

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
        info!("Client bootstrap with peer {bootstrap_peers:?}");
        Client::new(secret_key, bootstrap_peers, true, None, None)
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
        info!("Getting {wallet_balance} tokens from the faucet...");
        let tokens = send(from, wallet_balance, local_wallet.address(), client, true).await?;

        println!("Verifying the transfer from faucet...");
        info!("Verifying the transfer from faucet...");
        client.verify_cashnote(&tokens).await?;
        local_wallet.deposit_and_store_to_disk(&vec![tokens])?;
        assert_eq!(local_wallet.balance(), wallet_balance);
        println!("CashNotes deposited to the wallet that'll pay for storage: {wallet_balance}.");
        info!("CashNotes deposited to the wallet that'll pay for storage: {wallet_balance}.");

        Ok(local_wallet)
    }

    async fn load_faucet_wallet() -> Result<LocalWallet> {
        info!("Loading faucet...");
        let now = Instant::now();
        for attempt in 1..LOAD_FAUCET_WALLET_RETRIES + 1 {
            let faucet_wallet = create_faucet_wallet();

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
}

struct Droplet;
impl Droplet {
    /// Create a new client and bootstrap from the provided safe_peers
    pub async fn get_gossip_client(inventory: &DeploymentInventory) -> Client {
        let secret_key = bls::SecretKey::random();

        let mut bootstrap_peers = Vec::new();
        for peer in inventory
            .peers
            .iter()
            .chain(vec![&inventory.genesis_multiaddr])
        {
            match parse_peer_addr(peer) {
                Ok(peer) => bootstrap_peers.push(peer),
                Err(err) => error!("Can't parse SAFE_PEERS {peer:?} with error {err:?}"),
            }
        }
        if bootstrap_peers.is_empty() {
            panic!("Could parse/find any bootstrap peers");
        }

        println!("Client bootstrap with peer {bootstrap_peers:?}");
        info!("Client bootstrap with peer {bootstrap_peers:?}");
        Client::new(secret_key, Some(bootstrap_peers), true, None, None)
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
        info!(
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
                    error!("Failed to parse transfer: {err:?}");
                    error!("Transfer: \"{transfer}\"");
                    return Err(err.into());
                }
            };
            let cashnotes = match client.receive(&transfer, &local_wallet).await {
                Ok(cashnotes) => cashnotes,
                Err(err) => {
                    println!("Failed to verify and redeem transfer: {err:?}");
                    error!("Failed to verify and redeem transfer: {err:?}");
                    return Err(err.into());
                }
            };
            println!("Successfully verified transfer.");
            info!("Successfully verified transfer.");
            local_wallet.deposit_and_store_to_disk(&cashnotes)?;
        }

        Ok(local_wallet)
    }
}
