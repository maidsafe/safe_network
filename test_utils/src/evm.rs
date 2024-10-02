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

use crate::env_from_runtime_or_compiletime;

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
            env_from_runtime_or_compiletime!("RPC_URL").expect("`RPC_URL` not set"),
            env_from_runtime_or_compiletime!("PAYMENT_TOKEN_ADDRESS")
                .expect("`PAYMENT_TOKEN_ADDRESS` not set"),
            env_from_runtime_or_compiletime!("CHUNK_PAYMENTS_ADDRESS")
                .expect("`CHUNK_PAYMENTS_ADDRESS` not set"),
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

    let private_key = env_from_runtime_or_compiletime!("EVM_PRIVATE_KEY").unwrap_or_else(|| {
        "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80".to_string()
    });

    evmlib::wallet::Wallet::new_from_private_key(network, &private_key)
        .expect("Invalid private key")
}
