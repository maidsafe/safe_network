// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use sn_peers_acquisition::parse_peer_addr;

pub mod evm;
pub mod testnet;

// Get environment variable from runtime or build time, in that order. Returns `None` if not set.
macro_rules! env_from_runtime_or_compiletime {
    ($var:literal) => {{
        if let Ok(val) = std::env::var($var) {
            Some(val)
        } else if let Some(val) = option_env!($var) {
            Some(val.to_string())
        } else {
            None
        }
    }};
}

pub(crate) use env_from_runtime_or_compiletime;
use libp2p::Multiaddr;

/// Parse the `SAFE_PEERS` env var into a list of Multiaddrs.
///
/// An empty `Vec` will be returned if the env var is not set.
pub fn peers_from_env() -> Result<Vec<Multiaddr>, libp2p::multiaddr::Error> {
    let Some(peers_str) = env_from_runtime_or_compiletime!("SAFE_PEERS") else {
        return Ok(vec![]);
    };

    peers_str.split(',').map(parse_peer_addr).collect()
}
