use safenode::{
    comms::{Comm, NetworkNode},
    error::Result,
    stableset::{run_stable_set, StableSetMsg},
};
use tokio::io::AsyncWriteExt;

use std::collections::BTreeSet;
use std::path::Path;
use std::{fs, net::SocketAddr};
use tokio::fs::File;

#[macro_use]
extern crate tracing;

const PEERS_CONFIG_FILE: &str = "peers.json";

/// Read my addr from env var and peers addr from config file
fn peers_from_json(path: impl AsRef<Path>) -> Result<BTreeSet<SocketAddr>> {
    let peers_json = match fs::read_to_string(path) {
        Ok(peers_string) => peers_string,
        Err(error) => {
            warn!("Reading json file: {error:?}, using empty peers");
            return Ok(BTreeSet::default());
        }
    };
    let peers_ip_str: Vec<String> = serde_json::from_str(&peers_json)?;
    let peers_addr: BTreeSet<SocketAddr> = peers_ip_str
        .iter()
        .map(|p| p.parse().expect("Unable to parse socket address"))
        .collect();
    info!("Read Peers from config: {:?}", peers_addr);
    Ok(peers_addr)
}

/// start node and no_return unless fatal error
/// chooses a random port for the node
/// if no peers are supplied, assumes we are starting a fresh network
///
/// TODO: proper error handling here
async fn start_node(peers_addrs: BTreeSet<SocketAddr>) -> Result<()> {
    info!("Starting a new node");
    let peers: BTreeSet<_> = peers_addrs
        .into_iter()
        .map(|p| NetworkNode { addr: p })
        .collect();

    let is_first_node = peers.is_empty();

    let (comm, comm_event_receiver) = Comm::new::<StableSetMsg>().expect("Comms Failed");

    let my_addr = comm.socket_addr();
    let myself = NetworkNode { addr: my_addr };

    if is_first_node {
        info!("Starting as the genesis node");

        let our_config_file = vec![my_addr];
        let json = serde_json::to_string(&our_config_file)?;

        let mut file = File::create(PEERS_CONFIG_FILE).await?;
        file.write(json.as_bytes()).await?;
    }

    info!("Started comms for node {my_addr:?}");

    info!("Run stable set with peers {peers:?}");
    run_stable_set(comm, comm_event_receiver, myself, peers).await
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let peers_addr = peers_from_json(PEERS_CONFIG_FILE)?;

    start_node(peers_addr).await?;

    Ok(())
}
