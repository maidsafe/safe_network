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
use bytes::Bytes;
use safenode_proto::{safe_node_client::SafeNodeClient, NodeEventsRequest, NodeInfoRequest};

use eyre::Result;
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
use tracing::{debug, error};
use tracing_core::Level;

const NODE_COUNT: u32 = 25;
const CHUNKS_SIZE: usize = 1024 * 1024;

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
        LogOutputDest::Path(tmp_dir.join("safe-client")),
        LogFormat::Default,
    )?;

    // state
    let stored_chunks = Arc::new(RwLock::new(BTreeMap::new()));

    let all_peers = rpc_to_nodes(stored_chunks.clone()).await?;
    println!("all peers {all_peers:?}");

    // Store chunks
    let client = get_client().await;
    let file_api = Files::new(client);
    for _ in 0..10 {
        let chunk_addr = store_chunk(&file_api, stored_chunks.clone()).await?;
        let key = RecordKey::new(chunk_addr.name());

        let record_key = KBucketKey::from(key.to_vec());
        let expected_closest_peers: Vec<_> = sort_peers_by_key(
            all_peers.iter().cloned().collect(),
            &record_key,
            CLOSE_GROUP_SIZE,
        )?;

        tokio::time::sleep(Duration::from_secs(1)).await;
        let closest = stored_chunks.read().await.get(&chunk_addr).unwrap().clone();
        println!("expected {expected_closest_peers:?}\ngot{closest:?}");
    }

    tokio::time::sleep(Duration::from_secs(10)).await;
    Ok(())
}

/// Spawn a thread for each running node to handle the `NodeEvent` they emit
/// returns all the PeerId
async fn rpc_to_nodes(
    stored_chunks: Arc<RwLock<BTreeMap<ChunkAddress, BTreeSet<PeerId>>>>,
) -> Result<BTreeSet<PeerId>> {
    let mut all_peers = BTreeSet::new();

    let mut addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12000);
    for node_index in 1..NODE_COUNT {
        addr.set_port(12000 + node_index as u16);
        let endpoint = format!("https://{addr}");
        let mut rpc_client = SafeNodeClient::connect(endpoint).await?;

        // get the peer_id
        let response = rpc_client
            .node_info(Request::new(NodeInfoRequest {}))
            .await?;
        let peer_id = PeerId::from_bytes(&response.get_ref().peer_id)?;
        all_peers.insert(peer_id);

        // handle NodeEvent
        let stored_chunks_clone = stored_chunks.clone();
        let _handle: JoinHandle<Result<()>> = tokio::spawn(async move {
            let event_rx = rpc_client
                .node_events(Request::new(NodeEventsRequest {}))
                .await?;
            if let Some(event_bytes) = event_rx.into_inner().message().await? {
                if let Ok(NodeEvent::ChunkStored(chunk_addr)) =
                    NodeEvent::from_bytes(&event_bytes.event)
                {
                    if let Some(value) = stored_chunks_clone.write().await.get_mut(&chunk_addr) {
                        value.insert(peer_id);
                    } else {
                        error!("{peer_id:?}: stored_chunks_clone does not contain the chunk_addr {chunk_addr:?}");
                    }
                }
            }
            Ok(())
        });
    }

    Ok(all_peers)
}

// Generate a random Chunk and store it to the Network
// Returns the ChunkAddress
async fn store_chunk(
    file_api: &Files,
    stored_chunks: Arc<RwLock<BTreeMap<ChunkAddress, BTreeSet<PeerId>>>>,
) -> Result<ChunkAddress> {
    let mut rng = OsRng;
    let random_bytes: Vec<u8> = ::std::iter::repeat(())
        .map(|()| rng.gen::<u8>())
        .take(CHUNKS_SIZE)
        .collect();
    let bytes = Bytes::copy_from_slice(&random_bytes);

    let addr = ChunkAddress::new(
        file_api
            .calculate_address(bytes.clone())
            .expect("Failed to calculate new Chunk address"),
    );
    debug!("inserting chunk addr {addr:?} into the map");
    match stored_chunks.write().await.entry(addr) {
        Entry::Vacant(entry) => entry.insert(BTreeSet::new()),
        Entry::Occupied(_) => panic!("chunk should be new"),
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
