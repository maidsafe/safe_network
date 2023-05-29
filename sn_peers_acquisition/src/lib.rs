// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.
#[macro_use]
extern crate tracing;

use eyre::{eyre, Result};
use libp2p::{multiaddr::Protocol, Multiaddr, PeerId};
use std::env::current_exe;

const SAFE_PEERS: &str = "SAFE_PEERS";

/// Parse multiaddresses containing the P2p protocol (`/p2p/<PeerId>`).
/// Returns an error for the first invalid multiaddress.
fn parse_peer_multiaddresses(multiaddrs: &[Multiaddr]) -> Result<Vec<(PeerId, Multiaddr)>> {
    multiaddrs
        .iter()
        .map(|multiaddr| {
            // Take hash from the `/p2p/<hash>` component.
            let p2p_multihash = multiaddr
                .iter()
                .find_map(|p| match p {
                    Protocol::P2p(hash) => Some(hash),
                    _ => None,
                })
                .ok_or_else(|| eyre!("address does not contain `/p2p/<PeerId>`"))?;
            // Parse the multihash into the `PeerId`.
            let peer_id =
                PeerId::from_multihash(p2p_multihash).map_err(|_| eyre!("invalid p2p PeerId"))?;

            Ok((peer_id, multiaddr.clone()))
        })
        // Short circuit on the first error. See rust docs `Result::from_iter`.
        .collect::<Result<Vec<(PeerId, Multiaddr)>>>()
}

/// Grab contact peers from the command line or from the SAFE_PEERS env var.
/// The command line argument has priority over the env var.
/// Only one of these will be used as the source of peers
pub fn peers_from_opts_or_env(opt_peers: &[Multiaddr]) -> Result<Vec<(PeerId, Multiaddr)>> {
    let peers = parse_peer_multiaddresses(opt_peers)?;

    if !peers.is_empty() {
        info!("Using passed peers to contact the network: {:?}", peers);
        return Ok(peers);
    }

    let mut peers = vec![];
    if let Ok(env_peers) = std::env::var(SAFE_PEERS) {
        for peer in env_peers.split(',') {
            peers.push(peer.parse()?);
        }
    }

    if !peers.is_empty() {
        info!(
            "Using contact peers from $SAFE_PEERS env var to intitiate contact with the network: {:?}",
            peers
        );

        return parse_peer_multiaddresses(&peers);
    } else if !cfg!(feature = "local-discovery") {
        warn!("No {SAFE_PEERS} env var found. As `local-discovery` feature is disabled, we will not be able to connect to the network. ");

        // get the current bin name
        let current_crate_path = current_exe()?;

        let current_crate = current_crate_path.file_name().and_then(|s| s.to_str());

        // if we're a client let's bail early here
        // node impls won't bail as it could be a first node eg, and otheres will contact this node
        if let Some(crate_name) = current_crate {
            if crate_name == "safe" {
                return Err(eyre!("No {SAFE_PEERS} env var found. As `local-discovery` feature is disabled, we will not be able to connect to the network. "));
            }
        }
    } else {
        info!("No {SAFE_PEERS} env var found. As `local-discovery` feature is enabled, we will be attempt to connect to the network using mDNS.");
    }

    Ok(vec![])
}
