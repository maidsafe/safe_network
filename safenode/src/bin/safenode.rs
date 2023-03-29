mod log;

use log::init_node_logging;
use safenode::{
    comms::{Comm, NetworkNode},
    error::Result,
    // stableset::{run_stable_set, StableSetMsg},
};
use tokio::io::AsyncWriteExt;

use std::path::Path;
use std::{collections::BTreeSet, path::PathBuf};
use std::{
    fs,
    net::{IpAddr, Ipv4Addr, SocketAddr},
};
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

    let my_addr = {
        let addr = comm.socket_addr();
        // convert 0.0.0.0 -> 127.0.0.1.
        // i'm not sure why its pulling in 0000...
        if addr.ip() == IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)) {
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), addr.port())
        } else {
            addr
        }
    };

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
    // run_stable_set(comm, comm_event_receiver, myself, peers).await
}

/// Grabs the log dir arg if passed in
fn grab_log_dir() -> Option<PathBuf> {
    let mut args = std::env::args().skip(1); // Skip the first argument (the program name)

    let mut log_dir = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--log-dir" => {
                log_dir = args.next();
            }
            _ => {
                println!("Unknown argument: {}", arg);
            }
        }
    }

    if let Some(log_dir) = log_dir {
        Some(PathBuf::from(log_dir))
    } else {
        None
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // tracing_subscriber::fmt::init();
    let log_dir = grab_log_dir();
    let _log_appender_guard = init_node_logging(&log_dir)?;

    let peers_addr = peers_from_json(PEERS_CONFIG_FILE)?;

    start_node(peers_addr).await?;

    Ok(())
}
