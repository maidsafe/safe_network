// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use evmlib::{utils::evm_network_from_env, Network};
use std::env;

pub fn get_funded_wallet() -> evmlib::wallet::Wallet {
    let network =
        evm_network_from_env().expect("Failed to get EVM network from environment variables");
    if matches!(network, Network::ArbitrumOne) {
        panic!("You're trying to use ArbitrumOne network. Use a custom network for testing.");
    }
    // Default deployer wallet of the testnet.
    const DEFAULT_WALLET_PRIVATE_KEY: &str =
        "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

    let private_key = env::var("EVM_PRIVATE_KEY").unwrap_or(DEFAULT_WALLET_PRIVATE_KEY.to_string());

    evmlib::wallet::Wallet::new_from_private_key(network, &private_key)
        .expect("Invalid private key")
}
