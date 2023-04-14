// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use safenode::{
    log::init_node_logging,
    network::Network,
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

    info!("Starting a node...");
    let node_events_channel = Node::run(socket_addr).await?;

    let mut node_events_rx = node_events_channel.subscribe();
    if let Ok(event) = node_events_rx.recv().await {
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
}

// Todo: Implement node bootstrapping to connect to peers from outside the local network
#[allow(dead_code)]
async fn bootstrap_node(network_api: &mut Network, addr: Multiaddr) -> Result<()> {
    let peer_id = match addr.iter().last() {
        Some(Protocol::P2p(hash)) => PeerId::from_multihash(hash).expect("Valid hash."),
        _ => return Err(eyre!("Expect peer multiaddr to contain peer ID.")),
    };
    network_api
        .dial(peer_id, addr)
        .await
        .expect("Dial to succeed");
    Ok(())
}
