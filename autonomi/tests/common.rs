// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.
#![allow(dead_code)]

use bytes::Bytes;
use evmlib::{utils::get_evm_network_from_env, wallet::Wallet, Network};
use eyre::{bail, Context, Result};
use libp2p::Multiaddr;
use rand::Rng;
use sn_peers_acquisition::parse_peer_addr;
use std::env;

/// Generate random data of the given length.
pub fn gen_random_data(len: usize) -> Bytes {
    let mut data = vec![0u8; len];
    rand::thread_rng().fill(&mut data[..]);
    Bytes::from(data)
}

/// Parse the `SAFE_PEERS` env var into a list of Multiaddrs.
///
/// An empty `Vec` will be returned if the env var is not set or if local discovery is enabled.
pub fn peers_from_env() -> Result<Vec<Multiaddr>> {
    let bootstrap_peers = if cfg!(feature = "local") {
        Ok(vec![])
    } else if let Ok(peers_str) = std::env::var("SAFE_PEERS") {
        peers_str.split(',').map(parse_peer_addr).collect()
    } else {
        Ok(vec![])
    }?;
    Ok(bootstrap_peers)
}

pub fn get_funded_wallet() -> evmlib::wallet::Wallet {
    let network =
        get_evm_network_from_env().expect("Failed to get EVM network from environment variables");
    if matches!(network, Network::ArbitrumOne) {
        panic!("You're trying to use ArbitrumOne network. Use a custom network for testing.");
    }
    // Default deployer wallet of the testnet.
    const DEFAULT_WALLET_PRIVATE_KEY: &str =
        "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

    let private_key = env::var("SECRET_KEY").unwrap_or(DEFAULT_WALLET_PRIVATE_KEY.to_string());

    Wallet::new_from_private_key(network, &private_key).expect("Invalid private key")
}

pub fn get_new_wallet() -> Result<Wallet> {
    let network = get_evm_network_from_env()
        .wrap_err("Failed to get EVM network from environment variables")?;
    if matches!(network, Network::ArbitrumOne) {
        bail!("You're trying to use ArbitrumOne network. Use a custom network for testing.");
    }

    Ok(Wallet::new_with_random_wallet(network))
}
