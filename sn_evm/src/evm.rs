// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::EvmNetwork;

pub use evmlib::utils::{DATA_PAYMENTS_ADDRESS, PAYMENT_TOKEN_ADDRESS, RPC_URL};

/// Load the evm network from env
pub fn network_from_env() -> EvmNetwork {
    let rpc_url = std::env::var(RPC_URL);
    let payment_token_address = std::env::var(PAYMENT_TOKEN_ADDRESS);
    let data_payments_address = std::env::var(DATA_PAYMENTS_ADDRESS);

    match (rpc_url, payment_token_address, data_payments_address) {
        // all parameters are custom
        (Ok(url), Ok(tok), Ok(pay)) => EvmNetwork::new_custom(&url, &tok, &pay),
        // only rpc url is custom
        (Ok(url), _, _) => {
            let defaults = EvmNetwork::ArbitrumOne;
            let tok = defaults.payment_token_address().to_string();
            let pay = defaults.data_payments_address().to_string();
            EvmNetwork::new_custom(&url, &tok, &pay)
        }
        // default to arbitrum one
        _ => EvmNetwork::ArbitrumOne,
    }
}
