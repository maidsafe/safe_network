// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use eyre::{bail, OptionExt, Result};
use lazy_static::lazy_static;
use libp2p::PeerId;
use sn_client::{acc_packet::load_account_wallet_or_create_with_mnemonic, send, Client};
use sn_peers_acquisition::parse_peer_addr;
use sn_protocol::safenode_proto::{NodeInfoRequest, RestartRequest};
use sn_service_management::{
    get_local_node_registry_path, safenode_manager_proto::NodeServiceRestartRequest, NodeRegistry,
};
use sn_transfers::{create_faucet_wallet, HotWallet, NanoTokens, Transfer};
use std::{net::SocketAddr, path::Path};
use test_utils::testnet::DeploymentInventory;
use tokio::{
    sync::Mutex,
    time::{Duration, Instant},
};
use tonic::Request;
use tracing::{debug, error, info, warn};

use crate::common::get_safenode_rpc_client;

use super::get_safenode_manager_rpc_client;

/// This is a limited hard coded value as Droplet version has to contact the faucet to get the funds.
/// This is limited to 10 requests to the faucet, where each request yields 100 SNT
pub const INITIAL_WALLET_BALANCE: u64 = 3 * 100 * 1_000_000_000;

/// 100 SNT is added when `add_funds_to_wallet` is called.
/// This is limited to 1 request to the faucet, where each request yields 100 SNT
pub const ADD_FUNDS_TO_WALLET: u64 = 100 * 1_000_000_000;

/// The node count for a locally running network that the tests expect
pub const LOCAL_NODE_COUNT: usize = 25;
// The number of times to try to load the faucet wallet
const LOAD_FAUCET_WALLET_RETRIES: usize = 6;

lazy_static! {
    // mutex to restrict access to faucet wallet from concurrent tests
    static ref FAUCET_WALLET_MUTEX: Mutex<()> = Mutex::new(());
}

/// Load HotWallet from dir
pub fn get_wallet(root_dir: &Path) -> HotWallet {
    load_account_wallet_or_create_with_mnemonic(root_dir, None)
        .expect("Wallet shall be successfully created.")
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
                return Ok(inventory.rpc_endpoints.values().cloned().collect());
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

            let rpc_endpoints = inventory
                .rpc_endpoints
                .into_iter()
                .filter(|(_, addr)| addr.ip() != genesis_ip)
                .map(|(_, addr)| addr)
                .collect();
            Ok(rpc_endpoints)
        }
        Err(_) => {
            let local_node_reg_path = &get_local_node_registry_path()?;
            let local_node_registry = NodeRegistry::load(local_node_reg_path)?;
            let rpc_endpoints = local_node_registry
                .nodes
                .iter()
                .map(|n| n.rpc_socket_addr)
                .collect::<Vec<SocketAddr>>();
            Ok(rpc_endpoints)
        }
    }
}

/// Adds funds to the provided to_wallet_dir
/// If SN_INVENTORY flag is passed, the amount is retrieved from the faucet url
/// else obtain it from the provided faucet HotWallet
///
/// We obtain 100 SNT from the network per call. Use `get_client_and_wallet` during the initial setup which would
/// obtain 10*100 SNT
pub async fn add_funds_to_wallet(client: &Client, to_wallet_dir: &Path) -> Result<HotWallet> {
    match DeploymentInventory::load() {
        Ok(inventory) => {
            Droplet::get_funded_wallet(client, to_wallet_dir, inventory.faucet_address, false).await
        }
        Err(_) => NonDroplet::get_funded_wallet(client, to_wallet_dir, false).await,
    }
}

/// Create a client and fund the wallet.
/// If SN_INVENTORY flag is passed, the wallet is funded by fetching it from the faucet
/// Else create a genesis wallet and transfer funds from there.
///
/// We get a maximum of 10*100 SNT from the network. This is hardcoded as the Droplet tests have the fetch the
/// coins from the faucet and each request is limited to 100 SNT.
pub async fn get_client_and_funded_wallet(root_dir: &Path) -> Result<(Client, HotWallet)> {
    match DeploymentInventory::load() {
        Ok(inventory) => {
            let client = Droplet::get_client(&inventory).await;
            let local_wallet =
                Droplet::get_funded_wallet(&client, root_dir, inventory.faucet_address, true)
                    .await?;
            Ok((client, local_wallet))
        }
        Err(_) => {
            let client = NonDroplet::get_client().await;
            let local_wallet = NonDroplet::get_funded_wallet(&client, root_dir, true).await?;

            Ok((client, local_wallet))
        }
    }
}

pub struct NonDroplet;
impl NonDroplet {
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
        info!("Client bootstrap with peer {bootstrap_peers:?}");
        Client::new(secret_key, bootstrap_peers, None, None)
            .await
            .expect("Client shall be successfully created.")
    }

    pub async fn get_funded_wallet(
        client: &Client,
        root_dir: &Path,
        initial_wallet: bool,
    ) -> Result<HotWallet> {
        let wallet_balance = if initial_wallet {
            NanoTokens::from(INITIAL_WALLET_BALANCE)
        } else {
            NanoTokens::from(ADD_FUNDS_TO_WALLET)
        };
        let _guard = FAUCET_WALLET_MUTEX.lock().await;
        let from_faucet_wallet = NonDroplet::load_faucet_wallet().await?;
        let mut local_wallet = get_wallet(root_dir);

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

    async fn load_faucet_wallet() -> Result<HotWallet> {
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

    // Restart a local node by sending in the SafenodeRpcCmd::Restart to the node's RPC endpoint.
    pub async fn restart_node(rpc_endpoint: SocketAddr, retain_peer_id: bool) -> Result<()> {
        let mut rpc_client = get_safenode_rpc_client(rpc_endpoint).await?;

        let response = rpc_client
            .node_info(Request::new(NodeInfoRequest {}))
            .await?;
        let root_dir = Path::new(&response.get_ref().data_dir);
        debug!("Obtained root dir from node {root_dir:?}.");

        let record_store = root_dir.join("record_store");
        if record_store.exists() {
            println!("Removing content from the record store {record_store:?}");
            info!("Removing content from the record store {record_store:?}");
            std::fs::remove_dir_all(record_store)?;
        }
        let secret_key_file = root_dir.join("secret-key");
        if secret_key_file.exists() {
            println!("Removing secret-key file {secret_key_file:?}");
            info!("Removing secret-key file {secret_key_file:?}");
            std::fs::remove_file(secret_key_file)?;
        }
        let wallet_dir = root_dir.join("wallet");
        if wallet_dir.exists() {
            println!("Removing wallet dir {wallet_dir:?}");
            info!("Removing wallet dir {wallet_dir:?}");
            std::fs::remove_dir_all(wallet_dir)?;
        }

        let _response = rpc_client
            .restart(Request::new(RestartRequest {
                delay_millis: 0,
                retain_peer_id,
            }))
            .await?;

        println!("Node restart requested to RPC service at {rpc_endpoint}");
        info!("Node restart requested to RPC service at {rpc_endpoint}");
        Ok(())
    }
}

pub struct Droplet;
impl Droplet {
    /// Create a new client and bootstrap from the provided safe_peers
    pub async fn get_client(inventory: &DeploymentInventory) -> Client {
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
        Client::new(secret_key, Some(bootstrap_peers), None, None)
            .await
            .expect("Client shall be successfully created.")
    }

    // Create a wallet at root_dir and fetch the amount from the faucet url
    async fn get_funded_wallet(
        client: &Client,
        root_dir: &Path,
        faucet_socket: String,
        initial_wallet: bool,
    ) -> Result<HotWallet> {
        let _guard = FAUCET_WALLET_MUTEX.lock().await;

        let requests_to_faucet = if initial_wallet {
            let requests_to_faucet = 3;
            assert_eq!(
                requests_to_faucet * 100 * 1_000_000_000,
                INITIAL_WALLET_BALANCE
            );
            requests_to_faucet
        } else {
            let requests_to_faucet = 1;
            assert_eq!(
                requests_to_faucet * 100 * 1_000_000_000,
                ADD_FUNDS_TO_WALLET
            );
            requests_to_faucet
        };

        let mut local_wallet = get_wallet(root_dir);
        let address_hex = hex::encode(local_wallet.address().to_bytes());

        println!(
            "Getting {} tokens from the faucet... num_requests:{requests_to_faucet}",
            NanoTokens::from(INITIAL_WALLET_BALANCE)
        );
        info!(
            "Getting {} tokens from the faucet... num_requests:{requests_to_faucet}",
            NanoTokens::from(INITIAL_WALLET_BALANCE)
        );
        for _ in 0..requests_to_faucet {
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
            info!("Successfully verified transfer.");
            local_wallet.deposit_and_store_to_disk(&cashnotes)?;
        }
        println!(
            "Successfully got {} after {requests_to_faucet} requests to the faucet",
            NanoTokens::from(INITIAL_WALLET_BALANCE)
        );
        info!(
            "Successfully got {} after {requests_to_faucet} requests to the faucet",
            NanoTokens::from(INITIAL_WALLET_BALANCE)
        );

        Ok(local_wallet)
    }

    // Restart a remote safenode service by sending a RPC to the safenode manager daemon.
    pub async fn restart_node(
        peer_id: &PeerId,
        daemon_endpoint: SocketAddr,
        retain_peer_id: bool,
    ) -> Result<()> {
        let mut rpc_client = get_safenode_manager_rpc_client(daemon_endpoint).await?;

        let _response = rpc_client
            .restart_node_service(Request::new(NodeServiceRestartRequest {
                peer_id: peer_id.to_bytes(),
                delay_millis: 0,
                retain_peer_id,
            }))
            .await?;

        println!("Node restart requested to safenodemand {daemon_endpoint}");
        info!("Node restart requested to safenodemand {daemon_endpoint}");

        Ok(())
    }
}
