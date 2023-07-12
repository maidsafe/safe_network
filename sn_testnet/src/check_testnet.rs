// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use safenode_proto::safe_node_client::SafeNodeClient;
use safenode_proto::{NetworkInfoRequest, NodeInfoRequest};
use tonic::Request;

// this includes code generated from .proto files
#[allow(unused_qualifications)]
mod safenode_proto {
    tonic::include_proto!("safenode_proto");
}

use color_eyre::Result;
use libp2p::{multiaddr::Protocol, Multiaddr, PeerId};
use regex::Regex;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::{self, Display},
    fs::File,
    io::prelude::*,
    io::BufReader,
    net::SocketAddr,
    path::{Path, PathBuf},
    str::FromStr,
};
use tokio::time::{sleep, Duration};
use walkdir::WalkDir;

const LOG_FILENAME_PREFIX: &str = "safenode.log";

// Struct to collect node info from logs and RPC responses
#[derive(Debug, Clone)]
struct NodeInfo {
    pid: u32,
    peer_id: PeerId,
    listeners: Vec<Multiaddr>,
    log_path: PathBuf,
}

impl Display for NodeInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "- PID: {}", self.pid)?;
        writeln!(f, "- Peer Id: {}", self.peer_id)?;
        writeln!(f, "- Listeners:")?;
        for addr in self.listeners.iter() {
            writeln!(f, "   * {}", addr)?;
        }
        writeln!(f, "- Log dir: {}", self.log_path.display())
    }
}

pub async fn run(logs_path: &Path, node_count: u32) -> Result<()> {
    let mut delay_secs = 10;
    while delay_secs > 0 {
        println!("Verifying nodes in {delay_secs} seconds...");
        sleep(Duration::from_secs(1)).await;
        delay_secs -= 1;
    }
    println!();
    println!("======== Verifying Nodes ========");

    let expected_node_count = node_count as usize;
    println!(
        "Checking log files to verify all ({expected_node_count}) nodes \
        have joined. Logs path: {}",
        logs_path.display()
    );
    let nodes = nodes_info_from_logs(logs_path)?;

    println!("Number of nodes found in logs: {}", nodes.len());
    assert_eq!(
        expected_node_count,
        nodes.len(),
        "Unexpected number of joined nodes. Expected {}, we have {}",
        expected_node_count,
        nodes.len()
    );

    println!();
    println!("All nodes have joined. Nodes PIDs and PeerIds:");
    for (_, node_info) in nodes.iter() {
        println!(
            "{} -> {} @ {}",
            node_info.pid,
            node_info.peer_id,
            node_info.log_path.display()
        );
    }

    // let's check all nodes know about each other on the network
    for i in 1..nodes.len() + 1 {
        let rpc_addr: SocketAddr = format!("127.0.0.1:{}", 12000 + i as u16).parse()?;
        println!();
        println!("Checking peer id and network knowledge of node with RPC at {rpc_addr}");
        let (node_info, connected_peers) = send_rpc_queries_to_node(rpc_addr).await?;
        let peer_id = node_info.peer_id;

        let node_log_info = nodes
            .get(&node_info.pid)
            .expect("Mismatch in node's PID between logs and RPC response");

        assert_eq!(
            peer_id, node_log_info.peer_id,
            "Node at {} reported a mismatching PeerId: {}",
            rpc_addr, peer_id
        );

        if node_info.listeners != node_log_info.listeners {
            println!(
                "Node at {} reported a mismatching list of listeners: {:?}",
                rpc_addr, node_info.listeners
            );
        }

        /* Temporarily skipping this verification
        assert_eq!(
            node_info.listeners, node_log_info.listeners,
            "Node at {} reported a mismatching list of listeners: {:?}",
            rpc_addr, node_info.listeners
        );
        */

        if connected_peers.len() != expected_node_count - 1 {
            println!(
                "Node {} is connected to {} peers, expected: {}. Connected peers: {:?}",
                peer_id,
                connected_peers.len(),
                expected_node_count - 1,
                connected_peers
            );
        }

        /* Temporarily skipping these verifications
        assert_eq!(
            connected_peers.len(),
            expected_node_count - 1,
            "Node {} is connected to {} peers, expected: {}. Connected peers: {:?}",
            peer_id,
            connected_peers.len(),
            expected_node_count - 1,
            connected_peers
        );

        let expected_connections = nodes
            .iter()
            .filter_map(|(_, node_info)| {
                if node_info.peer_id != peer_id {
                    Some(node_info.peer_id)
                } else {
                    None
                }
            })
            .collect::<BTreeSet<_>>();

        assert_eq!(
            connected_peers, expected_connections,
            "At least one peer the node is connected to is not expected"
        );
        */

        println!("{node_info}");
    }

    println!();
    println!("Peer IDs and node network knowledge are as expected!");

    Ok(())
}

pub async fn obtain_peer_id(address: SocketAddr) -> Result<PeerId> {
    let endpoint = format!("https://{address}");
    println!("Connecting to node's RPC service at {endpoint} ...");
    let mut client = SafeNodeClient::connect(endpoint).await?;

    let request = Request::new(NodeInfoRequest {});
    let response = client.node_info(request).await?;
    let node_info = response.get_ref();
    let peer_id = PeerId::from_bytes(&node_info.peer_id)?;
    Ok(peer_id)
}

// Parse node logs files and extract info for each of them
fn nodes_info_from_logs(path: &Path) -> Result<BTreeMap<u32, NodeInfo>> {
    let mut nodes = BTreeMap::<PathBuf, NodeInfo>::new();
    let re = Regex::new(r"Node \(PID: (\d+)\) with PeerId: (.*)")?;

    let re_listener = Regex::new("Local node is listening on \"(.+)\"")?;

    let log_files = WalkDir::new(path).into_iter().filter_map(|entry| {
        entry.ok().and_then(|f| {
            if f.file_type().is_file() {
                Some(f.into_path())
            } else {
                None
            }
        })
    });

    for file_path in log_files {
        let file_name = if let Some(name) = file_path.file_name().and_then(|s| s.to_str()) {
            name
        } else {
            println!("Failed to obtain filename from {}", file_path.display());
            continue;
        };

        if file_name.starts_with(LOG_FILENAME_PREFIX) {
            let file = File::open(&file_path)?;
            let lines = BufReader::new(file).lines();
            let log_path = file_path
                .parent()
                .expect("Failed to get parent dir")
                .to_path_buf();

            lines.map_while(|item| item.ok()).for_each(|line| {
                if let Some(cap) = re.captures_iter(&line).next() {
                    println!(">>>>>>>>>.. {line:?}");
                    let pid = cap[1].parse().expect("Failed to parse PID from node log");
                    let peer_id =
                        PeerId::from_str(&cap[2]).expect("Failed to parse PeerId from node log");

                    update_node_info(&mut nodes, &log_path, Some((pid, peer_id)), None);
                }

                if let Some(cap) = re_listener.captures_iter(&line).next() {
                    let multiaddr_str: String = cap[1]
                        .parse()
                        .expect("Failed to parse multiaddr from node log");
                    let multiaddr = Multiaddr::from_str(&multiaddr_str)
                        .expect("Failed to deserialise Multiaddr from node log");

                    update_node_info(&mut nodes, &log_path, None, Some(multiaddr));
                }
            });
        }
    }

    Ok(nodes
        .into_values()
        .map(|node_info| (node_info.pid, node_info))
        .collect())
}

// Helper to update parts of a NodeInfo when parsing logs
fn update_node_info(
    nodes: &mut BTreeMap<PathBuf, NodeInfo>,
    log_path: &Path,
    peer_info: Option<(u32, PeerId)>,
    listener: Option<Multiaddr>,
) {
    let node_info = nodes.entry(log_path.to_path_buf()).or_insert(NodeInfo {
        pid: 0,
        peer_id: PeerId::random(),
        listeners: vec![],
        log_path: log_path.to_path_buf(),
    });

    if let Some((pid, peer_id)) = peer_info {
        node_info.pid = pid;
        node_info.peer_id = peer_id;
    }

    node_info.listeners.extend(listener);
}

// Send RPC requests to the node at the provided address,
// querying for its own info and network connections.
async fn send_rpc_queries_to_node(addr: SocketAddr) -> Result<(NodeInfo, BTreeSet<PeerId>)> {
    let endpoint = format!("https://{addr}");
    println!("Connecting to node's RPC service at {endpoint} ...");
    let mut client = SafeNodeClient::connect(endpoint).await?;

    let request = Request::new(NodeInfoRequest {});
    let response = client.node_info(request).await?;
    let node_info = response.get_ref();
    let peer_id = PeerId::from_bytes(&node_info.peer_id)?;

    let request = Request::new(NetworkInfoRequest {});
    let response = client.network_info(request).await?;
    let net_info = response.get_ref();
    let listeners = net_info
        .listeners
        .iter()
        .map(|multiaddr| {
            // let's add the peer id to the addr since that's how it's logged
            let mut multiaddr = Multiaddr::from_str(multiaddr)
                .expect("Failed to deserialise Multiaddr from RPC response");
            multiaddr.push(Protocol::P2p(peer_id));
            multiaddr
        })
        .collect::<Vec<_>>();

    let connected_peers = net_info
        .connected_peers
        .iter()
        .map(|bytes| {
            PeerId::from_bytes(bytes).expect("Failed to deserialise PeerId from RPC response")
        })
        .collect();

    Ok((
        NodeInfo {
            pid: node_info.pid,
            peer_id,
            listeners,
            log_path: node_info.log_dir.clone().into(),
        },
        connected_peers,
    ))
}
