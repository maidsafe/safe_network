#![allow(dead_code)]

use bytes::Bytes;
use libp2p::Multiaddr;
use rand::Rng;
use sn_client::acc_packet::load_account_wallet_or_create_with_mnemonic;
use sn_peers_acquisition::parse_peer_addr;
use sn_transfers::{get_faucet_data_dir, HotWallet};

/// When launching a testnet locally, we can use the faucet wallet.
pub fn load_hot_wallet_from_faucet() -> HotWallet {
    let root_dir = get_faucet_data_dir();
    load_account_wallet_or_create_with_mnemonic(&root_dir, None)
        .expect("faucet wallet should be available for tests")
}

pub fn gen_random_data(len: usize) -> Bytes {
    let mut data = vec![0u8; len];
    rand::thread_rng().fill(&mut data[..]);
    Bytes::from(data)
}

/// Enable logging for tests. E.g. use `RUST_LOG=autonomi` to see logs.
pub fn enable_logging() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();
}

/// Parse the `SAFE_PEERS` env var into a list of Multiaddrs.
///
/// An empty `Vec` will be returned if the env var is not set.
pub fn peers_from_env() -> Result<Vec<Multiaddr>, libp2p::multiaddr::Error> {
    let Ok(peers_str) = std::env::var("SAFE_PEERS") else {
        return Ok(vec![]);
    };

    peers_str.split(',').map(parse_peer_addr).collect()
}
