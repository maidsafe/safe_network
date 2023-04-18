// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use safenode::{
    log::init_node_logging,
    node::{Node, NodeEvent},
};

use clap::Parser;
use eyre::{eyre, Result};
use libp2p::{multiaddr::Protocol, Multiaddr, PeerId};
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
    thread, time,
};
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    let opt = Opt::parse();
    let _log_appender_guard = init_node_logging(&opt.log_dir)?;

    let socket_addr = SocketAddr::new(opt.ip, opt.port);
    let peers = parse_peer_multiaddreses(&opt.peers)?;

    info!("Starting a node...");
    let node_events_channel = Node::run(socket_addr, peers).await?;

    let mut node_events_rx = node_events_channel.subscribe();

    loop {
        let event = match node_events_rx.recv().await {
            Ok(e) => e,
            Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                tracing::error!("Node event channel closed!");
                break;
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                tracing::warn!("Skipped {n} node events!");
                continue;
            }
        };
        match event {
            NodeEvent::ConnectedToNetwork => {
                info!("Connected to the Network");
            }
        }
    }

    // Keep the node running.
    loop {
        thread::sleep(time::Duration::from_millis(100));
    }
}

#[derive(Parser, Debug)]
#[clap(name = "safenode cli")]
struct Opt {
    #[clap(long)]
    log_dir: Option<PathBuf>,

    /// Specify specific port to listen on.
    /// Defaults to 0, which means any available port.
    #[clap(long, default_value_t = 0)]
    port: u16,

    /// Specify specific IP to listen on.
    /// Defaults to 0.0.0.0, which will bind to all network interfaces.
    #[clap(long, default_value_t = IpAddr::V4(Ipv4Addr::UNSPECIFIED))]
    ip: IpAddr,

    /// Nodes we dial at start to help us get connected to the network. Can be specified multiple times.
    #[clap(long = "peer")]
    peers: Vec<Multiaddr>,
}

/// Parse multiaddresses containing the P2p protocol (`/p2p/<PeerId>`).
/// Returns an error for the first invalid multiaddress.
fn parse_peer_multiaddreses(multiaddrs: &[Multiaddr]) -> Result<Vec<(PeerId, Multiaddr)>> {
    multiaddrs
        .iter()
        .map(|multiaddr| {
            // Take hash from the `/p2p/<hash>` component.
            let p2p_multihash = multiaddr
                .iter()
                .find_map(|p| match p {
                    Protocol::P2p(hash) => Some(hash),
                    _ => None,
                })
                .ok_or_else(|| eyre!("address does not contain `/p2p/<PeerId>`"))?;
            // Parse the multihash into the `PeerId`.
            let peer_id =
                PeerId::from_multihash(p2p_multihash).map_err(|_| eyre!("invalid p2p PeerId"))?;

            Ok((peer_id, multiaddr.clone()))
        })
        // Short circuit on the first error. See rust docs `Result::from_iter`.
        .collect::<Result<Vec<(PeerId, Multiaddr)>>>()
}
