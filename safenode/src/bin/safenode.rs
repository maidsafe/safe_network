use safenode::comms::{Comm, NetworkNode};
use safenode::stableset::{run_stable_set, StableSetMsg};

use std::collections::BTreeSet;
use std::path::Path;
use std::{env, fs, net::SocketAddr};

const PEERS_CONFIG_FILE: &str = "peers.json";

/// Read my addr from env var and peers addr from config file
fn peers_from_json(path: impl AsRef<Path>) -> BTreeSet<SocketAddr> {
    let peers_json =
        fs::read_to_string(path).expect("Unable to read peers config file");
    let peers_ip_str: Vec<String> =
        serde_json::from_str(&peers_json).expect("Unable to parse peers config file");
    let peers_addr: BTreeSet<SocketAddr> = peers_ip_str
        .iter()
        .map(|p| p.parse().expect("Unable to parse socket address"))
        .collect();
    println!("Read Peers from config: {:?}", peers_addr);
    peers_addr
}

/// start node and no_return unless fatal error
async fn start_node(my_addr: SocketAddr, peers_addrs: BTreeSet<SocketAddr>) {
    println!("Starting comms for node {my_addr:?}");
    let peers = peers_addrs
        .into_iter()
        .map(|p| NetworkNode { addr: p })
        .collect();

    let (sender, receiver) = Comm::new::<StableSetMsg>(my_addr).expect("Comms Failed");
    let myself = NetworkNode { addr: my_addr };

    println!("Run stable set with peers {peers:?}");
    run_stable_set(sender, receiver, myself, peers).await
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // Simple parsing of single socket address argument.
    let my_addr = {
        let args: Vec<String> = env::args().collect();
        if args.len() != 2 {
            eprintln!("Missing argument\nusage: safenode <socket address>");
            return;
        }
        args[1].parse().expect("Unable to parse socket address")
    };

    let mut peers_addr = peers_from_json(PEERS_CONFIG_FILE);
    peers_addr.remove(&my_addr); // Remove our own address from our network list.

    start_node(my_addr, peers_addr).await;
}
