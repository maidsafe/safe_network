#![allow(dead_code)]

use autonomi::Wallet;
use bytes::Bytes;
use const_hex::ToHexExt;
use evmlib::{CustomNetwork, Network};
use rand::Rng;
use std::env;

fn get_var_or_panic(var: &str) -> String {
    env::var(var).expect(&format!("{} environment variable needs to be set", var))
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

pub fn evm_network_from_env() -> Network {
    let evm_network = env::var("EVM_NETWORK").ok();
    let arbitrum_flag = evm_network.as_deref() == Some("arbitrum-one");

    let (rpc_url, payment_token_address, chunk_payments_address) = if arbitrum_flag {
        (
            Network::ArbitrumOne.rpc_url().to_string(),
            Network::ArbitrumOne
                .payment_token_address()
                .encode_hex_with_prefix(),
            Network::ArbitrumOne
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

    Network::Custom(CustomNetwork::new(
        &rpc_url,
        &payment_token_address,
        &chunk_payments_address,
    ))
}

pub fn evm_wallet_from_env_or_default(network: Network) -> Wallet {
    // Default deployer wallet of the testnet.
    const DEFAULT_WALLET_PRIVATE_KEY: &str =
        "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

    let private_key = env::var("PRIVATE_KEY").unwrap_or(DEFAULT_WALLET_PRIVATE_KEY.to_string());

    Wallet::new_from_private_key(network, &private_key).expect("Invalid private key")
}
