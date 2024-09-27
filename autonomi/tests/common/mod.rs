#![allow(dead_code)]

use bytes::Bytes;
use const_hex::ToHexExt;
use evmlib::CustomNetwork;
use libp2p::Multiaddr;
use rand::Rng;
use sn_client::acc_packet::load_account_wallet_or_create_with_mnemonic;
use sn_peers_acquisition::parse_peer_addr;
use sn_transfers::{get_faucet_data_dir, HotWallet};
use std::env;

fn get_var_or_panic(var: &str) -> String {
    env::var(var).expect(&format!("{var} environment variable needs to be set"))
}

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
    let Ok(peers_str) = env::var("SAFE_PEERS") else {
        return Ok(vec![]);
    };

    peers_str.split(',').map(parse_peer_addr).collect()
}

pub fn evm_network_from_env() -> evmlib::Network {
    let evm_network = env::var("EVM_NETWORK").ok();
    let arbitrum_flag = evm_network.as_deref() == Some("arbitrum-one");

    let (rpc_url, payment_token_address, chunk_payments_address) = if arbitrum_flag {
        (
            evmlib::Network::ArbitrumOne.rpc_url().to_string(),
            evmlib::Network::ArbitrumOne
                .payment_token_address()
                .encode_hex_with_prefix(),
            evmlib::Network::ArbitrumOne
                .chunk_payments_address()
                .encode_hex_with_prefix(),
        )
    } else {
        (
            get_var_or_panic("RPC_URL"),
            get_var_or_panic("PAYMENT_TOKEN_ADDRESS"),
            get_var_or_panic("CHUNK_PAYMENTS_ADDRESS"),
        )
    };

    evmlib::Network::Custom(CustomNetwork::new(
        &rpc_url,
        &payment_token_address,
        &chunk_payments_address,
    ))
}

pub fn evm_wallet_from_env_or_default(network: evmlib::Network) -> evmlib::wallet::Wallet {
    // Default deployer wallet of the testnet.
    const DEFAULT_WALLET_PRIVATE_KEY: &str =
        "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

    let private_key = env::var("PRIVATE_KEY").unwrap_or(DEFAULT_WALLET_PRIVATE_KEY.to_string());

    evmlib::wallet::Wallet::new_from_private_key(network, &private_key)
        .expect("Invalid private key")
}
