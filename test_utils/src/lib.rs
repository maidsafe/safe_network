// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

pub mod evm;
pub mod testnet;

use bytes::Bytes;
use color_eyre::eyre::Result;
use libp2p::Multiaddr;
use rand::Rng;
use sn_peers_acquisition::parse_peer_addr;

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
    } else if let Some(peers_str) = env_from_runtime_or_compiletime!("SAFE_PEERS") {
        peers_str.split(',').map(parse_peer_addr).collect()
    } else {
        Ok(vec![])
    }?;
    Ok(bootstrap_peers)
}
