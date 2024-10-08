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
    match evmlib::utils::evm_network_from_env() {
        Ok(network) => network,
        Err(e) => {
            warn!("Failed to get EVM network from environment variables, using default: {e}");
            EvmNetwork::default()
        }
    }
}
