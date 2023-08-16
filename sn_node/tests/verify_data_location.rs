// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

#![allow(clippy::mutable_key_type)]
mod common;

use assert_fs::TempDir;
use bytes::Bytes;
use common::{
    get_client_and_wallet, node_restart,
    safenode_proto::{safe_node_client::SafeNodeClient, NodeInfoRequest, RecordAddressesRequest},
    PAYING_WALLET_INITIAL_BALANCE,
};
use eyre::{eyre, Result};
use libp2p::{
    kad::{KBucketKey, RecordKey},
    PeerId,
};
use rand::{rngs::OsRng, Rng};
use sn_client::{Client, Files, WalletClient};
use sn_logging::{init_logging, LogFormat, LogOutputDest};
use sn_networking::{sort_peers_by_key, CLOSE_GROUP_SIZE};
use sn_protocol::{storage::ChunkAddress, NetworkAddress, PrettyPrintRecordKey};
use sn_transfers::wallet::LocalWallet;
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Duration,
};
use tonic::Request;
use tracing_core::Level;

const NODE_COUNT: u8 = 25;
const CHUNK_SIZE: usize = 1024;

// VERIFICATION_DELAY is set based on the dead peer detection interval
// Once a node has been restarted, it takes VERIFICATION_DELAY time
// for the old peer to be removed from the routing table.
// Replication is then kicked off to distribute the data to the new closest
// nodes, hence verification has to be performed after this.
const VERIFICATION_DELAY: Duration = Duration::from_secs(300);

// Number of times to retry verification if it fails
const VERIFICATION_ATTEMPTS: usize = 3;

// Default number of churns that should be performed. After each churn, we
// wait for VERIFICATION_DELAY time before verifying the data location.
// It can be overridden by setting the 'CHURN_COUNT' env var.
const CHURN_COUNT: u8 = 4;

/// Default number of chunks that should be PUT to the netowrk.
// It can be overridden by setting the 'CHUNK_COUNT' env var.
const CHUNK_COUNT: usize = 5;

type NodeIndex = u8;
type RecordHolders = HashMap<RecordKey, HashSet<NodeIndex>>;

#[tokio::test(flavor = "multi_thread")]
async fn verify_data_location() -> Result<()> {
    let tmp_dir = std::env::temp_dir();
    let logging_targets = vec![
        ("safenode".to_string(), Level::TRACE),
        ("sn_transfers".to_string(), Level::TRACE),
        ("sn_networking".to_string(), Level::TRACE),
        ("sn_node".to_string(), Level::TRACE),
    ];
    let _log_appender_guard = init_logging(
        logging_targets,
        LogOutputDest::Path(tmp_dir.to_path_buf()),
        LogFormat::Default,
    )?;

    let churn_count = if let Ok(str) = std::env::var("CHURN_COUNT") {
        str.parse::<u8>()?
    } else {
        CHURN_COUNT
    };
    let chunk_count = if let Ok(str) = std::env::var("CHUNK_COUNT") {
        str.parse::<usize>()?
    } else {
        CHUNK_COUNT
    };
    println!(
        "Performing data location verification with a churn count of {churn_count} and n_chunks {chunk_count}\nIt will take approx {:?}",
        VERIFICATION_DELAY*churn_count as u32
    );

    // set of all the node indexes that stores a record key
    let mut record_holders = RecordHolders::default();
    let mut all_peers = get_all_peer_ids().await?;

    // Store chunks
    println!("Creating a client and paying wallet...");
    let paying_wallet_dir = TempDir::new()?;

    let (client, paying_wallet) =
        get_client_and_wallet(paying_wallet_dir.path(), PAYING_WALLET_INITIAL_BALANCE).await?;

    store_chunk(client, paying_wallet, chunk_count).await?;
    get_initial_record_keys(&mut record_holders).await?;

    // Verify data location initially
    get_record_holder_list(&mut record_holders).await?;
    verify_location(&record_holders, &all_peers).await?;

    // Churn nodes and verify the location of the data after VERIFICATION_DELAY
    let mut addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12000);
    let mut node_index = 1;
    let mut current_churn_count = 0;

    loop {
        if current_churn_count >= churn_count {
            break Ok(());
        }
        current_churn_count += 1;

        // restart a node
        addr.set_port(12000 + node_index);
        node_restart(addr).await?;

        // wait for the dead peer to be removed from the RT and the replication flow to finish
        println!("\nNode {node_index} has been restarted, waiting for {VERIFICATION_DELAY:?} before verification");
        tokio::time::sleep(VERIFICATION_DELAY).await;

        // get the new PeerId for the current NodeIndex
        let endpoint = format!("https://{addr}");
        let mut rpc_client = SafeNodeClient::connect(endpoint).await?;
        let response = rpc_client
            .node_info(Request::new(NodeInfoRequest {}))
            .await?;
        let peer_id = PeerId::from_bytes(&response.get_ref().peer_id)?;
        all_peers[node_index as usize - 1] = peer_id;

        // get the new set of holders
        get_record_holder_list(&mut record_holders).await?;

        print_node_close_groups(&all_peers);

        verify_location(&record_holders, &all_peers).await?;

        node_index += 1;
        if node_index > NODE_COUNT as u16 {
            node_index = 1;
        }
    }
}

fn print_node_close_groups(all_peers: &[PeerId]) {
    let all_peers = all_peers.to_vec();
    println!("\nNode close groups:");
    for (node_index, peer) in all_peers.iter().enumerate() {
        let node_index = node_index + 1;
        let key = NetworkAddress::from_peer(*peer).as_kbucket_key();
        let closest_peers = sort_peers_by_key(all_peers.clone(), &key, CLOSE_GROUP_SIZE)
            .expect("failed to sort peer");
        let closest_peers_idx = closest_peers
            .iter()
            .map(|peer| all_peers.iter().position(|p| p == peer).unwrap() + 1)
            .collect::<Vec<_>>();
        println!("Close for {node_index}: {peer:?} are {closest_peers_idx:?}");
    }
}

// Get initial set of records keys after put
async fn get_initial_record_keys(record_holders: &mut RecordHolders) -> Result<()> {
    for node_index in 1..NODE_COUNT + 1 {
        let mut addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12000);
        addr.set_port(12000 + node_index as u16);
        let endpoint = format!("https://{addr}");
        let mut rpc_client = SafeNodeClient::connect(endpoint).await?;

        let response = rpc_client
            .record_addresses(Request::new(RecordAddressesRequest {}))
            .await?;

        for bytes in response.get_ref().addresses.iter() {
            let key = RecordKey::from(bytes.clone());
            record_holders.insert(key, Default::default());
        }
    }
    Ok(())
}

async fn get_record_holder_list(record_holders: &mut RecordHolders) -> Result<()> {
    // Clear the set of NodeIndex before updating with the new set
    for (_, v) in record_holders.iter_mut() {
        *v = HashSet::new();
    }
    for node_index in 1..NODE_COUNT + 1 {
        let mut addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12000);
        addr.set_port(12000 + node_index as u16);
        let endpoint = format!("https://{addr}");
        let mut rpc_client = SafeNodeClient::connect(endpoint).await?;

        let response = rpc_client
            .record_addresses(Request::new(RecordAddressesRequest {}))
            .await?;

        for bytes in response.get_ref().addresses.iter() {
            let key = RecordKey::from(bytes.clone());
            record_holders
                .get_mut(&key)
                .ok_or_else(|| eyre!("Key {key:?} has not been PUT to the network by the test. Please restart the local testnet"))?
                .insert(node_index);
        }
    }
    println!("Obtained the current set of Record Key holders");
    Ok(())
}

// Verifies that the chunk is stored by the actual closest peers to the RecordKey
async fn verify_location(record_holders: &RecordHolders, all_peers: &[PeerId]) -> Result<()> {
    let mut failed = HashMap::new();
    let mut verification_attempts = 0;
    while verification_attempts < VERIFICATION_ATTEMPTS {
        failed.clear();
        for (key, actual_closest_idx) in record_holders.iter() {
            println!("Verifying {:?}", PrettyPrintRecordKey::from(key.clone()));
            let record_key = KBucketKey::from(key.to_vec());
            let expected_closest_peers =
                sort_peers_by_key(all_peers.to_vec(), &record_key, CLOSE_GROUP_SIZE)?
                    .into_iter()
                    .collect::<BTreeSet<_>>();

            let actual_closest = actual_closest_idx
                .iter()
                .map(|idx| all_peers[*idx as usize - 1])
                .collect::<BTreeSet<_>>();

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
            record_holders.iter().for_each(|(key, node_index)| {
                println!(
                    "Record {:?} is currently held by node indexes {node_index:?}",
                    PrettyPrintRecordKey::from(key.clone())
                );
            });
            println!("Node index map:");
            all_peers
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
        println!("Verification failed after {VERIFICATION_ATTEMPTS} times");
        Err(eyre!("Verification failed for: {failed:?}"))
    } else {
        println!("All the Records have been verified!");
        Ok(())
    }
}

// Returns all the PeerId for all the locally running nodes
async fn get_all_peer_ids() -> Result<Vec<PeerId>> {
    let mut all_peers = Vec::new();

    let mut addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12000);
    for node_index in 1..NODE_COUNT + 1 {
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
    println!("Obtained the PeerId list for the locally running network with a node count of {NODE_COUNT}");
    Ok(all_peers)
}

// Generate a random Chunk and store it to the Network
async fn store_chunk(client: Client, paying_wallet: LocalWallet, chunk_count: usize) -> Result<()> {
    let mut rng = OsRng;
    let mut wallet_client = WalletClient::new(client.clone(), paying_wallet);
    let file_api = Files::new(client);

    let mut uploaded_chunks_count = 0;
    loop {
        if uploaded_chunks_count >= chunk_count {
            break;
        }

        let random_bytes: Vec<u8> = ::std::iter::repeat(())
            .map(|()| rng.gen::<u8>())
            .take(CHUNK_SIZE)
            .collect();
        let bytes = Bytes::copy_from_slice(&random_bytes);

        let (addr, chunks) = file_api
            .chunk_bytes(bytes.clone())
            .expect("Failed to chunk bytes");

        println!(
            "Paying storage for ({}) new Chunk/s of file ({} bytes) at {addr:?}",
            chunks.len(),
            bytes.len()
        );

        let (proofs, _) = wallet_client
            .pay_for_storage(chunks.iter().map(|c| c.network_address()), true)
            .await
            .expect("Failed to pay for storage for new file at {addr:?}");

        println!(
            "Storing ({}) Chunk/s of file ({} bytes) at {addr:?}",
            chunks.len(),
            bytes.len()
        );

        let addr = ChunkAddress::new(file_api.calculate_address(bytes.clone())?);
        let key = PrettyPrintRecordKey::from(RecordKey::new(addr.xorname()));
        file_api.upload_with_transfers(bytes, &proofs, true).await?;
        uploaded_chunks_count += 1;

        println!("Stored Chunk with {addr:?} / {key:?}");
    }

    // to make sure the last chunk was stored
    tokio::time::sleep(Duration::from_secs(10)).await;

    Ok(())
}
