// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use bytes::Bytes;
use eyre::Result;
use libp2p::Multiaddr;
use rand::Rng;
use sn_peers_acquisition::parse_peer_addr;
use std::env;

#[allow(dead_code)]
pub fn gen_random_data(len: usize) -> Bytes {
    let mut data = vec![0u8; len];
    rand::thread_rng().fill(&mut data[..]);
    Bytes::from(data)
}

#[allow(dead_code)]
/// Enable logging for tests. E.g. use `RUST_LOG=autonomi` to see logs.
pub fn enable_logging() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();
}

#[allow(dead_code)]
/// Parse the `SAFE_PEERS` env var into a list of Multiaddrs.
///
/// An empty `Vec` will be returned if the env var is not set.
pub fn peers_from_env() -> Result<Vec<Multiaddr>> {
    let bootstrap_peers = if cfg!(feature = "local-discovery") {
        Ok(vec![])
    } else if let Ok(peers_str) = env::var("SAFE_PEERS") {
        peers_str.split(',').map(parse_peer_addr).collect()
    } else {
        Ok(vec![])
    }?;
    Ok(bootstrap_peers)
}
