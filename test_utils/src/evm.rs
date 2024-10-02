// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use const_hex::ToHexExt;
use evmlib::CustomNetwork;
use std::env;

fn get_var_or_panic(var: &str) -> String {
    env::var(var).unwrap_or_else(|_| panic!("{var} environment variable needs to be set"))
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

pub fn get_funded_wallet() -> evmlib::wallet::Wallet {
    let network = evm_network_from_env();
    // Default deployer wallet of the testnet.
    const DEFAULT_WALLET_PRIVATE_KEY: &str =
        "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

    let private_key = env::var("EVM_PRIVATE_KEY").unwrap_or(DEFAULT_WALLET_PRIVATE_KEY.to_string());

    evmlib::wallet::Wallet::new_from_private_key(network, &private_key)
        .expect("Invalid private key")
}
