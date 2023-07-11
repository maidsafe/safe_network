// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

// this includes code generated from .proto files
#[allow(unused_qualifications, unreachable_pub, clippy::unwrap_used)]
mod safenode_proto {
    tonic::include_proto!("safenode_proto");
}
mod common;

use common::node_restart;
use safenode_proto::{safe_node_client::SafeNodeClient, NodeEventsRequest, NodeInfoRequest};

use bytes::Bytes;
use eyre::{eyre, Result};
use libp2p::{
    kad::{KBucketKey, RecordKey},
    PeerId,
};
use rand::{rngs::OsRng, Rng};
use sn_client::{Client, Files};
use sn_logging::{init_logging, LogFormat, LogOutputDest};
use sn_networking::{sort_peers_by_key, CLOSE_GROUP_SIZE};
use sn_node::NodeEvent;
use sn_peers_acquisition::parse_peer_addr;
use sn_protocol::storage::ChunkAddress;
use std::{
    collections::{btree_map::Entry, BTreeMap, BTreeSet},
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
    time::Duration,
};
use tokio::{sync::RwLock, task::JoinHandle};
use tonic::Request;
use tracing_core::Level;

const NODE_COUNT: u8 = 25;
const CHUNK_SIZE: usize = 1024;
const CHUNK_COUNT: usize = 10;
const VERIFICATION_DELAY: Duration = Duration::from_secs(10);

type NodeIndex = u8;

type StoredChunks = Arc<RwLock<BTreeMap<ChunkAddress, BTreeSet<NodeIndex>>>>;

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

    // state
    let stored_chunks = Arc::new(RwLock::new(BTreeMap::new()));

    // spawn node event handler for each node
    for node_index in 1..NODE_COUNT + 1 {
        handle_node_events(stored_chunks.clone(), node_index).await?;
    }
    let mut all_peers = get_all_peer_ids().await?;

    // Store chunks
    let client = get_client().await;
    let file_api = Files::new(client);
    for _ in 0..CHUNK_COUNT {
        store_chunk(&file_api, stored_chunks.clone()).await?;
    }

    tokio::time::sleep(VERIFICATION_DELAY).await;
    verify_location(stored_chunks.clone(), &all_peers).await?;

    // churn all nodes and verify the location
    let mut addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12000);
    for node_index in 1..NODE_COUNT + 1 {
        // 1 is used as the bootstrap peer
        if node_index == 1 {
            continue;
        }
        addr.set_port(12000 + node_index as u16);

        let endpoint = format!("https://{addr}");
        let mut rpc_client = SafeNodeClient::connect(endpoint).await?;
        let response = rpc_client
            .node_info(Request::new(NodeInfoRequest {}))
            .await?;
        let peer_id_old = PeerId::from_bytes(&response.get_ref().peer_id)?;

        node_restart(addr).await?;
        println!("starting node event handler");
        while (handle_node_events(stored_chunks.clone(), node_index).await).is_err() {
            println!("Node RPC is not functional yet");
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        println!("Sleeping for {VERIFICATION_DELAY:?} before verifying");
        tokio::time::sleep(VERIFICATION_DELAY).await;

        let endpoint = format!("https://{addr}");
        let mut rpc_client = SafeNodeClient::connect(endpoint).await?;
        let response = rpc_client
            .node_info(Request::new(NodeInfoRequest {}))
            .await?;
        let peer_id = PeerId::from_bytes(&response.get_ref().peer_id)?;

        let old_one = all_peers.get(node_index as usize - 1).unwrap();
        assert_eq!(*old_one, peer_id_old);

        all_peers[node_index as usize - 1] = peer_id;
        verify_location(stored_chunks.clone(), &all_peers).await?;
    }

    Ok(())
}

// Verfies that the chunk is stored by the actual closest peers to the ChunkAddress
async fn verify_location(stored_chunks: StoredChunks, all_peers: &[PeerId]) -> Result<()> {
    for (idx, peer) in all_peers.iter().enumerate() {
        println!("{}: {peer:?}", idx + 1);
    }
    for chunk_addr in stored_chunks.read().await.keys() {
        let key = RecordKey::new(chunk_addr.name());

        let record_key = KBucketKey::from(key.to_vec());
        let expected_closest_peers =
            sort_peers_by_key(all_peers.to_vec(), &record_key, CLOSE_GROUP_SIZE)?
                .into_iter()
                .collect::<BTreeSet<_>>();

        let actual_closest_idx = stored_chunks.read().await.get(chunk_addr).unwrap().clone();
        let actual_closest = actual_closest_idx
            .iter()
            .map(|idx| all_peers[*idx as usize - 1])
            .collect::<BTreeSet<_>>();

        for expected in &expected_closest_peers {
            if !actual_closest.contains(expected) {
                return Err(eyre!(
                    "Chunk {chunk_addr:?} is not stored inside {expected:?}"
                ));
            }
        }
    }
    Ok(())
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
    Ok(all_peers)
}

/// Spawn a thread for each running node to handle the `NodeEvent` they emit
/// Keeps track of all the nodes that are storing a Chunk
async fn handle_node_events(
    stored_chunks: Arc<RwLock<BTreeMap<ChunkAddress, BTreeSet<NodeIndex>>>>,
    node_index: u8,
) -> Result<()> {
    let mut addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12000);
    addr.set_port(12000 + node_index as u16);
    let endpoint = format!("https://{addr}");
    let mut rpc_client = SafeNodeClient::connect(endpoint).await?;

    let stored_chunks_clone = stored_chunks.clone();
    let _handle: JoinHandle<Result<()>> = tokio::spawn(async move {
        loop {
            let event_rx = rpc_client
                .node_events(Request::new(NodeEventsRequest {}))
                .await?;
            let event_bytes =
                event_rx.into_inner().message().await?.ok_or_else(|| {
                    eyre!("Error while obtaining node event on node {node_index:?}")
                })?;
            let event = NodeEvent::from_bytes(&event_bytes.event)?;
            println!("Node: {node_index} Got node_event {event:?}");
            if let NodeEvent::ChunkStored(chunk_addr) = event {
                if let Some(value) = stored_chunks_clone.write().await.get_mut(&chunk_addr) {
                    value.insert(node_index);
                } else {
                    println!("stored_chunks does not contain the chunk_addr {chunk_addr:?}");
                }
            }
        }
    });

    Ok(())
}

// Generate a random Chunk and store it to the Network
// Returns the ChunkAddress
async fn store_chunk(file_api: &Files, stored_chunks: StoredChunks) -> Result<ChunkAddress> {
    let mut rng = OsRng;
    let random_bytes: Vec<u8> = ::std::iter::repeat(())
        .map(|()| rng.gen::<u8>())
        .take(CHUNK_SIZE)
        .collect();
    let bytes = Bytes::copy_from_slice(&random_bytes);

    let addr = ChunkAddress::new(
        file_api
            .calculate_address(bytes.clone())
            .expect("Failed to calculate new Chunk address"),
    );
    println!("Storing Chunk with addr {addr:?}");
    match stored_chunks.write().await.entry(addr) {
        Entry::Vacant(entry) => entry.insert(BTreeSet::new()),
        Entry::Occupied(_) => panic!("Chunk addr {addr:?} has been inserted into the map already"),
    };

    file_api
        .upload_with_proof(bytes, &BTreeMap::default())
        .await?;

    Ok(addr)
}

//  Get a new Client for testing
async fn get_client() -> Client {
    let secret_key = bls::SecretKey::random();

    let bootstrap_peers = if !cfg!(feature = "local-discovery") {
        match std::env::var("SAFE_PEERS") {
            Ok(str) => match parse_peer_addr(&str) {
                Ok(peer) => Some(vec![peer]),
                Err(err) => panic!("Cann't parse SAFE_PEERS {str:?} with error {err:?}"),
            },
            Err(err) => panic!("Cann't get env var SAFE_PEERS with error {err:?}"),
        }
    } else {
        None
    };
    println!("Client bootstrap with peer {bootstrap_peers:?}");
    Client::new(secret_key, bootstrap_peers, None)
        .await
        .expect("Client shall be successfully created.")
}
