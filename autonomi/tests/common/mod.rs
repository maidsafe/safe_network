#![allow(dead_code)]

use bytes::Bytes;
use evmlib::{CustomNetwork, Network};
use rand::Rng;
use std::env;

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
    let rpc_url = env::var("RPC_URL").expect("RPC_URL environment variable needs to be set.");
    let payment_token_address = env::var("PAYMENT_TOKEN_ADDRESS")
        .expect("PAYMENT_TOKEN_ADDRESS environment variable needs to be set.");
    let chunk_payments_address = env::var("CHUNK_PAYMENTS_ADDRESS")
        .expect("CHUNK_PAYMENTS_ADDRESS environment variable needs to be set.");

    Network::Custom(CustomNetwork::new(
        &rpc_url,
        &payment_token_address,
        &chunk_payments_address,
    ))
}
