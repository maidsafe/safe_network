// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use autonomi::Client;
use eyre::Result;
use sn_protocol::safenode_proto::{NodeInfoRequest, RestartRequest};
use sn_service_management::{get_local_node_registry_path, NodeRegistry};
use std::{net::SocketAddr, path::Path};
use test_utils::testnet::DeploymentInventory;
use test_utils::{evm::get_funded_wallet, peers_from_env};
use tokio::sync::Mutex;
use tonic::Request;
use tracing::{debug, info};

use crate::common::get_safenode_rpc_client;

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

// mutex to restrict access to faucet wallet from concurrent tests
static FAUCET_WALLET_MUTEX: Mutex<()> = Mutex::const_new(());

pub async fn get_client_and_funded_wallet() -> (Client, evmlib::wallet::Wallet) {
    match DeploymentInventory::load() {
        Ok(_inventory) => {
            todo!("Not implemented yet for WanNetwork");
        }
        Err(_) => (
            LocalNetwork::get_client().await,
            LocalNetwork::get_funded_wallet(),
        ),
    }
}

/// Get the node count
/// If SN_INVENTORY flag is passed, the node count is obtained from the the droplet
/// else return the local node count
pub fn get_node_count() -> usize {
    match DeploymentInventory::load() {
        Ok(_inventory) => {
            todo!("Not implemented yet for WanNetwork");
            // inventory.rpc_endpoints.len()
        }
        Err(_) => LOCAL_NODE_COUNT,
    }
}

/// Get the list of all RPC addresses
/// If SN_INVENTORY flag is passed, the RPC addresses of all the droplet nodes are returned
/// else generate local addresses for NODE_COUNT nodes
///
/// The genesis address is skipped for droplets as we don't want to restart the Genesis node there.
/// The restarted node relies on the genesis multiaddr to bootstrap after restart.
pub fn get_all_rpc_addresses(_skip_genesis_for_droplet: bool) -> Result<Vec<SocketAddr>> {
    match DeploymentInventory::load() {
        Ok(_inventory) => {
            todo!("Not implemented yet for WanNetwork");
            // if !skip_genesis_for_droplet {
            //     return Ok(inventory.rpc_endpoints.values().cloned().collect());
            // }
            // // else filter out genesis
            // let genesis_ip = inventory
            //     .vm_list
            //     .iter()
            //     .find_map(|(name, addr)| {
            //         if name.contains("genesis") {
            //             Some(*addr)
            //         } else {
            //             None
            //         }
            //     })
            //     .ok_or_eyre("Could not get the genesis VM's addr")?;

            // let rpc_endpoints = inventory
            //     .rpc_endpoints
            //     .into_iter()
            //     .filter(|(_, addr)| addr.ip() != genesis_ip)
            //     .map(|(_, addr)| addr)
            //     .collect();
            // Ok(rpc_endpoints)
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

// /// Adds funds to the provided to_wallet_dir
// /// If SN_INVENTORY flag is passed, the amount is retrieved from the faucet url
// /// else obtain it from the provided faucet HotWallet
// ///
// /// We obtain 100 SNT from the network per call. Use `get_client_and_wallet` during the initial setup which would
// /// obtain 10*100 SNT
// pub async fn add_funds_to_wallet(client: &Client, to_wallet_dir: &Path) -> Result<HotWallet> {
//     match DeploymentInventory::load() {
//         Ok(inventory) => {
//             Droplet::get_funded_wallet(client, to_wallet_dir, inventory.faucet_address, false).await
//         }
//         Err(_) => NonDroplet::get_funded_wallet(client, to_wallet_dir, false).await,
//     }
// }

pub struct LocalNetwork;
impl LocalNetwork {
    ///  Get a new Client for testing
    pub async fn get_client() -> Client {
        let bootstrap_peers = peers_from_env().expect("Failed to get bootstrap peers from env");

        println!("Client bootstrap with peer {bootstrap_peers:?}");
        info!("Client bootstrap with peer {bootstrap_peers:?}");
        Client::connect(&bootstrap_peers)
            .await
            .expect("Client shall be successfully created.")
    }

    fn get_funded_wallet() -> evmlib::wallet::Wallet {
        get_funded_wallet()
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

pub struct WanNetwork;
impl WanNetwork {
    // /// Create a new client and bootstrap from the provided safe_peers
    // pub async fn get_client(inventory: &DeploymentInventory) -> Client {
    //     let secret_key = bls::SecretKey::random();

    //     let mut bootstrap_peers = Vec::new();
    //     for peer in inventory
    //         .peers
    //         .iter()
    //         .chain(vec![&inventory.genesis_multiaddr])
    //     {
    //         match parse_peer_addr(peer) {
    //             Ok(peer) => bootstrap_peers.push(peer),
    //             Err(err) => error!("Can't parse SAFE_PEERS {peer:?} with error {err:?}"),
    //         }
    //     }
    //     if bootstrap_peers.is_empty() {
    //         panic!("Could parse/find any bootstrap peers");
    //     }

    //     println!("Client bootstrap with peer {bootstrap_peers:?}");
    //     info!("Client bootstrap with peer {bootstrap_peers:?}");
    //     Client::new(secret_key, Some(bootstrap_peers), None, None)
    //         .await
    //         .expect("Client shall be successfully created.")
    // }

    // // Create a wallet at root_dir and fetch the amount from the faucet url
    // async fn get_funded_wallet(
    //     client: &Client,
    //     root_dir: &Path,
    //     faucet_socket: String,
    //     initial_wallet: bool,
    // ) -> Result<HotWallet> {
    //     let _guard = FAUCET_WALLET_MUTEX.lock().await;

    //     let requests_to_faucet = if initial_wallet {
    //         let requests_to_faucet = 3;
    //         assert_eq!(
    //             requests_to_faucet * 100 * 1_000_000_000,
    //             INITIAL_WALLET_BALANCE
    //         );
    //         requests_to_faucet
    //     } else {
    //         let requests_to_faucet = 1;
    //         assert_eq!(
    //             requests_to_faucet * 100 * 1_000_000_000,
    //             ADD_FUNDS_TO_WALLET
    //         );
    //         requests_to_faucet
    //     };

    //     let mut local_wallet = get_wallet(root_dir);
    //     let address_hex = hex::encode(local_wallet.address().to_bytes());

    //     println!(
    //         "Getting {} tokens from the faucet... num_requests:{requests_to_faucet}",
    //         NanoTokens::from(INITIAL_WALLET_BALANCE)
    //     );
    //     info!(
    //         "Getting {} tokens from the faucet... num_requests:{requests_to_faucet}",
    //         NanoTokens::from(INITIAL_WALLET_BALANCE)
    //     );
    //     for _ in 0..requests_to_faucet {
    //         let faucet_url = format!("http://{faucet_socket}/{address_hex}");

    //         // Get transfer from faucet
    //         let transfer = reqwest::get(&faucet_url).await?.text().await?;
    //         let transfer = match Transfer::from_hex(&transfer) {
    //             Ok(transfer) => transfer,
    //             Err(err) => {
    //                 println!("Failed to parse transfer: {err:?}");
    //                 println!("Transfer: \"{transfer}\"");
    //                 error!("Failed to parse transfer: {err:?}");
    //                 error!("Transfer: \"{transfer}\"");
    //                 return Err(err.into());
    //             }
    //         };
    //         let cashnotes = match client.receive(&transfer, &local_wallet).await {
    //             Ok(cashnotes) => cashnotes,
    //             Err(err) => {
    //                 println!("Failed to verify and redeem transfer: {err:?}");
    //                 error!("Failed to verify and redeem transfer: {err:?}");
    //                 return Err(err.into());
    //             }
    //         };
    //         info!("Successfully verified transfer.");
    //         local_wallet.deposit_and_store_to_disk(&cashnotes)?;
    //     }
    //     println!(
    //         "Successfully got {} after {requests_to_faucet} requests to the faucet",
    //         NanoTokens::from(INITIAL_WALLET_BALANCE)
    //     );
    //     info!(
    //         "Successfully got {} after {requests_to_faucet} requests to the faucet",
    //         NanoTokens::from(INITIAL_WALLET_BALANCE)
    //     );

    //     Ok(local_wallet)
    // }

    // // Restart a remote safenode service by sending a RPC to the safenode manager daemon.
    // pub async fn restart_node(
    //     peer_id: &PeerId,
    //     daemon_endpoint: SocketAddr,
    //     retain_peer_id: bool,
    // ) -> Result<()> {
    //     let mut rpc_client = get_safenode_manager_rpc_client(daemon_endpoint).await?;

    //     let _response = rpc_client
    //         .restart_node_service(Request::new(NodeServiceRestartRequest {
    //             peer_id: peer_id.to_bytes(),
    //             delay_millis: 0,
    //             retain_peer_id,
    //         }))
    //         .await?;

    //     println!("Node restart requested to safenodemand {daemon_endpoint}");
    //     info!("Node restart requested to safenodemand {daemon_endpoint}");

    //     Ok(())
    // }
}
