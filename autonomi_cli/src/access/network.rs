// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use autonomi::EvmNetwork;
use autonomi::Multiaddr;
use color_eyre::eyre::Context;
use color_eyre::Result;
use color_eyre::Section;
use sn_peers_acquisition::PeersArgs;

use sn_peers_acquisition::SAFE_PEERS_ENV;

pub async fn get_peers(peers: PeersArgs) -> Result<Vec<Multiaddr>> {
    peers.get_peers().await
        .wrap_err("Please provide valid Network peers to connect to")
        .with_suggestion(|| format!("make sure you've provided network peers using the --peers option or the {SAFE_PEERS_ENV} env var"))
        .with_suggestion(|| "a peer address looks like this: /ip4/42.42.42.42/udp/4242/quic-v1/p2p/B64nodePeerIDvdjb3FAJF4ks3moreBase64CharsHere")
}

pub fn get_evm_network_from_env() -> Result<EvmNetwork> {
    #[cfg(feature = "local")]
    {
        println!("Getting EVM network from local CSV as the local feature is enabled");
        let network = autonomi::evm::local_evm_network_from_csv()
            .wrap_err("Failed to get EVM network from local CSV")
            .with_suggestion(|| "make sure you've set up the local EVM network by running `cargo run --bin evm_testnet`")?;
        Ok(network)
    }
    #[cfg(not(feature = "local"))]
    {
        let network = autonomi::evm::network_from_env();
        if matches!(network, EvmNetwork::Custom(_)) {
            println!("Using custom EVM network found from environment variables");
            info!("Using custom EVM network found from environment variables {network:?}");
        }
        Ok(network)
    }
}
