mod comms;
mod stableset;

use crate::comms::{Comm, NetworkNode};
use crate::stableset::{run_stable_set, StableSetMsg};

use std::collections::BTreeSet;
use std::{env, fs, net::SocketAddr};

const PEERS_CONFIG_FILE: &str = "peers.json";

/// Read my addr from env var and peers addr from config file
fn get_config() -> (SocketAddr, BTreeSet<SocketAddr>) {
    let my_addr_str: String = env::var("NODE_ADDR").expect("Failed to read NODE_ADDR from env");
    let my_addr = my_addr_str.parse().expect("Unable to parse socket address");
    let peers_json =
        fs::read_to_string(PEERS_CONFIG_FILE).expect("Unable to read peers config file");
    let peers_ip_str: Vec<String> =
        serde_json::from_str(&peers_json).expect("Unable to parse peers config file");
    let peers_addr: BTreeSet<SocketAddr> = peers_ip_str
        .iter()
        .filter(|p| *p != &my_addr_str)
        .map(|p| p.parse().expect("Unable to parse socket address"))
        .collect();
    println!("Read Peers from config: {:?}", peers_addr);
    (my_addr, peers_addr)
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

    let (my_addr, peers_addr) = get_config();

    start_node(my_addr, peers_addr).await;
}
