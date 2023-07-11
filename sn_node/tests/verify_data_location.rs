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
use safenode_proto::{safe_node_client::SafeNodeClient, NodeInfoRequest, RecordAddressesRequest};

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
use sn_peers_acquisition::parse_peer_addr;
use sn_protocol::storage::ChunkAddress;
use std::{
    collections::{hash_map::Entry, BTreeMap, BTreeSet, HashMap, HashSet},
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Duration,
};
use tonic::Request;
use tracing_core::Level;

const NODE_COUNT: u8 = 25;
const CHUNK_SIZE: usize = 1024;
const CHUNK_COUNT: usize = 10;
const VERIFICATION_DELAY: Duration = Duration::from_secs(10);

type NodeIndex = u8;

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

    let mut stored_chunks = HashMap::new();
    let mut all_peers = get_all_peer_ids().await?;

    // Store chunks
    let client = get_client().await;
    let file_api = Files::new(client);
    for _ in 0..CHUNK_COUNT {
        store_chunk(&file_api, &mut stored_chunks).await?;
    }

    get_all_addrs(&mut stored_chunks).await?;

    println!("stored_chunks {stored_chunks:?}");
    verify_location(&mut stored_chunks, &all_peers).await?;

    // churn all nodes and verify the location
    let mut addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12000);
    for node_index in 1..NODE_COUNT + 1 {
        // clear all the keys
        for (_, v) in stored_chunks.iter_mut() {
            *v = HashSet::new();
        }
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
    }
    tokio::time::sleep(Duration::from_secs(30)).await;
    get_all_addrs(&mut stored_chunks).await?;
    println!("stored_chunks {stored_chunks:?}");
    verify_location(&mut stored_chunks, &all_peers).await?;

    Ok(())
}

async fn get_all_addrs(stored_chunks: &mut HashMap<RecordKey, HashSet<NodeIndex>>) -> Result<()> {
    for node_index in 1..NODE_COUNT + 1 {
        println!("getting addresses for {node_index:?}");
        let mut addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12000);
        addr.set_port(12000 + node_index as u16);
        let endpoint = format!("https://{addr}");
        let mut rpc_client = SafeNodeClient::connect(endpoint).await?;

        let response = rpc_client
            .record_addresses(Request::new(RecordAddressesRequest {}))
            .await?;

        for bytes in response.get_ref().addresses.iter() {
            let key = libp2p::kad::RecordKey::from(bytes.clone());
            stored_chunks
                .get_mut(&key)
                .expect("Key {key:?} should be present")
                .insert(node_index);
        }
    }
    Ok(())
}

// Verfies that the chunk is stored by the actual closest peers to the ChunkAddress
async fn verify_location(
    stored_chunks: &mut HashMap<RecordKey, HashSet<NodeIndex>>,
    all_peers: &[PeerId],
) -> Result<()> {
    for (idx, peer) in all_peers.iter().enumerate() {
        println!("{}: {peer:?}", idx + 1);
    }
    for key in stored_chunks.keys() {
        let record_key = KBucketKey::from(key.to_vec());
        let expected_closest_peers =
            sort_peers_by_key(all_peers.to_vec(), &record_key, CLOSE_GROUP_SIZE)?
                .into_iter()
                .collect::<BTreeSet<_>>();

        let actual_closest_idx = stored_chunks.get(key).unwrap().clone();
        let actual_closest = actual_closest_idx
            .iter()
            .map(|idx| all_peers[*idx as usize - 1])
            .collect::<BTreeSet<_>>();

        for expected in &expected_closest_peers {
            if !actual_closest.contains(expected) {
                return Err(eyre!("Record {key:?} is not stored inside {expected:?}"));
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

// Generate a random Chunk and store it to the Network
// Returns the ChunkAddress
async fn store_chunk(
    file_api: &Files,
    stored_chunks: &mut HashMap<RecordKey, HashSet<NodeIndex>>,
) -> Result<ChunkAddress> {
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
    let key = RecordKey::new(addr.name());
    println!("Storing Chunk with addr {addr:?}, {key:?}");
    match stored_chunks.entry(key) {
        Entry::Vacant(entry) => entry.insert(HashSet::new()),
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
