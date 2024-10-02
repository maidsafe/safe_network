use bytes::Bytes;
use libp2p::Multiaddr;
use rand::Rng;
use sn_peers_acquisition::parse_peer_addr;
use std::env;

#[allow(dead_code)]
pub fn gen_random_data(len: usize) -> Bytes {
    let mut data = vec![0u8; len];
    rand::thread_rng().fill(&mut data[..]);
    Bytes::from(data)
}

#[allow(dead_code)]
/// Enable logging for tests. E.g. use `RUST_LOG=autonomi` to see logs.
pub fn enable_logging() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();
}

#[allow(dead_code)]
/// Parse the `SAFE_PEERS` env var into a list of Multiaddrs.
///
/// An empty `Vec` will be returned if the env var is not set.
pub fn peers_from_env() -> Result<Vec<Multiaddr>, libp2p::multiaddr::Error> {
    let Ok(peers_str) = env::var("SAFE_PEERS") else {
        return Ok(vec![]);
    };

    peers_str.split(',').map(parse_peer_addr).collect()
}
