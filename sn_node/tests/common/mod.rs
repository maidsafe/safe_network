// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

#![allow(dead_code)]
#![allow(clippy::mutable_key_type)]

#[allow(unused_qualifications, unreachable_pub, clippy::unwrap_used)]
pub mod safenode_proto {
    tonic::include_proto!("safenode_proto");
}

use eyre::{eyre, Result};
use lazy_static::lazy_static;
use libp2p::{
    kad::{KBucketKey, RecordKey},
    PeerId,
};
use safenode_proto::{
    safe_node_client::SafeNodeClient, NodeInfoRequest, RecordAddressesRequest, RestartRequest,
};
use sn_client::{load_faucet_wallet_from_genesis_wallet, send, Client};
use sn_dbc::Token;
use sn_logging::{LogFormat, LogOutputDest};
use sn_networking::{sort_peers_by_key, CLOSE_GROUP_SIZE};
use sn_peers_acquisition::parse_peer_addr;
use sn_protocol::PrettyPrintRecordKey;
use sn_transfers::wallet::LocalWallet;

use std::{
    collections::{HashMap, HashSet},
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::Path,
    sync::Once,
    time::Duration,
};
use tokio::{fs::remove_dir_all, sync::Mutex};
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
    Client::new(secret_key, bootstrap_peers, None)
        .await
        .expect("Client shall be successfully created.")
}

pub async fn get_wallet(root_dir: &Path) -> LocalWallet {
    LocalWallet::load_from(root_dir)
        .await
        .expect("Wallet shall be successfully created.")
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
    local_wallet.deposit(vec![tokens]);
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
        remove_dir_all(chunks_records).await?;
    }

    // remove Registers records
    let registers_records = root_dir.join("registers");
    if let Ok(true) = registers_records.try_exists() {
        println!(
            "Removing Registers records from {}",
            registers_records.display()
        );
        remove_dir_all(registers_records).await?;
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

// DATA_LOCATION_VERIFICATION_DELAY is set based on the dead peer detection interval
// Once a node has been restarted, it takes VERIFICATION_DELAY time
// for the old peer to be removed from the routing table.
// Replication is then kicked off to distribute the data to the new closest
// nodes, hence verification has to be performed after this.
pub const DATA_LOCATION_VERIFICATION_DELAY: Duration = Duration::from_secs(300);
// Number of times to retry verification if it fails
const DATA_LOCATION_VERIFICATION_ATTEMPTS: usize = 3;

type NodeIndex = usize;
pub struct DataLocationVerification {
    node_count: usize,
    pub all_peers: Vec<PeerId>,
    record_holders: HashMap<RecordKey, HashSet<NodeIndex>>,
}

impl DataLocationVerification {
    pub async fn new(node_count: usize) -> Result<Self> {
        let mut ver = Self {
            node_count,
            all_peers: Default::default(),
            record_holders: Default::default(),
        };
        ver.collect_all_peer_ids().await?;
        ver.collect_initial_record_keys().await?;
        Ok(ver)
    }

    pub fn update_peer_index(&mut self, node_index: NodeIndex, peer_id: PeerId) {
        self.all_peers[node_index - 1] = peer_id;
    }

    // Verifies that the chunk is stored by the actual closest peers to the RecordKey
    pub async fn verify(&mut self) -> Result<()> {
        self.get_record_holder_list().await?;

        let mut failed = HashMap::new();
        let mut verification_attempts = 0;
        while verification_attempts < DATA_LOCATION_VERIFICATION_ATTEMPTS {
            failed.clear();
            for (key, actual_closest_idx) in self.record_holders.iter() {
                println!("Verifying {:?}", PrettyPrintRecordKey::from(key.clone()));
                let record_key = KBucketKey::from(key.to_vec());
                let expected_closest_peers =
                    sort_peers_by_key(self.all_peers.clone(), &record_key, CLOSE_GROUP_SIZE)?
                        .into_iter()
                        .collect::<HashSet<_>>();

                let actual_closest = actual_closest_idx
                    .iter()
                    .map(|idx| self.all_peers[*idx - 1])
                    .collect::<HashSet<_>>();

                let mut failed_peers = Vec::new();
                expected_closest_peers
                    .iter()
                    .filter(|expected| !actual_closest.contains(expected))
                    .for_each(|expected| failed_peers.push(*expected));

                if !failed_peers.is_empty() {
                    failed.insert(key.clone(), failed_peers);
                }
            }

            if !failed.is_empty() {
                println!("Verification failed");

                failed.iter().for_each(|(key, failed_peers)| {
                    failed_peers.iter().for_each(|peer| {
                        println!(
                            "Record {:?} is not stored inside {peer:?}",
                            PrettyPrintRecordKey::from(key.clone()),
                        )
                    });
                });
                println!("State of each node:");
                self.record_holders.iter().for_each(|(key, node_index)| {
                    println!(
                        "Record {:?} is currently held by node indexes {node_index:?}",
                        PrettyPrintRecordKey::from(key.clone())
                    );
                });
                println!("Node index map:");
                self.all_peers
                    .iter()
                    .enumerate()
                    .for_each(|(idx, peer)| println!("{} : {peer:?}", idx + 1));
                verification_attempts += 1;
                println!("Sleeping before retrying verification");
                tokio::time::sleep(Duration::from_secs(20)).await;
            } else {
                // if successful, break out of the loop
                break;
            }
        }

        if !failed.is_empty() {
            println!("Verification failed after {DATA_LOCATION_VERIFICATION_ATTEMPTS} times");
            Err(eyre!("Verification failed for: {failed:?}"))
        } else {
            println!("All the Records have been verified!");
            Ok(())
        }
    }

    // Collect all the PeerId for all the locally running nodes
    async fn collect_all_peer_ids(&mut self) -> Result<()> {
        let mut all_peers = Vec::new();

        let mut addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12000);
        for node_index in 1..self.node_count + 1 {
            addr.set_port(12000 + node_index as u16);
            let endpoint = format!("https://{addr}");
            let mut rpc_client = SafeNodeClient::connect(endpoint).await?;

            // get the peer_id
            let response = rpc_client
                .node_info(Request::new(NodeInfoRequest {}))
                .await?;
            let peer_id = PeerId::from_bytes(&response.get_ref().peer_id)?;
            all_peers.push(peer_id);
        }
        println!(
            "Obtained the PeerId list for the locally running network with a node count of {}",
            self.node_count
        );

        self.all_peers = all_peers;
        Ok(())
    }

    // Collect the initial set of records keys after put
    async fn collect_initial_record_keys(&mut self) -> Result<()> {
        for node_index in 1..self.node_count + 1 {
            let mut addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12000);
            addr.set_port(12000 + node_index as u16);
            let endpoint = format!("https://{addr}");
            let mut rpc_client = SafeNodeClient::connect(endpoint).await?;

            let response = rpc_client
                .record_addresses(Request::new(RecordAddressesRequest {}))
                .await?;

            for bytes in response.get_ref().addresses.iter() {
                let key = RecordKey::from(bytes.clone());
                self.record_holders.insert(key, Default::default());
            }
        }
        Ok(())
    }

    // get all the current set of holders for pre filled Record keys
    async fn get_record_holder_list(&mut self) -> Result<()> {
        // Clear the set of NodeIndex before updating with the new set
        for (_, v) in self.record_holders.iter_mut() {
            *v = HashSet::new();
        }
        for node_index in 1..self.node_count + 1 {
            let mut addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12000);
            addr.set_port(12000 + node_index as u16);
            let endpoint = format!("https://{addr}");
            let mut rpc_client = SafeNodeClient::connect(endpoint).await?;

            let response = rpc_client
                .record_addresses(Request::new(RecordAddressesRequest {}))
                .await?;

            for bytes in response.get_ref().addresses.iter() {
                let key = RecordKey::from(bytes.clone());
                self.record_holders
                .get_mut(&key)
                .ok_or_else(|| eyre!("Key {key:?} has not been PUT to the network by the test. Please restart the local testnet"))?
                .insert(node_index);
            }
        }
        println!("Obtained the current set of Record Key holders");
        Ok(())
    }
}
