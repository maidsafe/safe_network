// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use std::env;

use bytes::Bytes;
use eyre::Result;
use libp2p::Multiaddr;
use rand::Rng;
use sn_peers_acquisition::parse_peer_addr;

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

#[cfg(target_arch = "wasm32")]
#[allow(dead_code)]
pub fn enable_logging_wasm(directive: impl AsRef<str>) {
    use tracing_subscriber::prelude::*;

    console_error_panic_hook::set_once();

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false) // Only partially supported across browsers
        .without_time() // std::time is not available in browsers
        .with_writer(tracing_web::MakeWebConsoleWriter::new()); // write events to the console
    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(tracing_subscriber::EnvFilter::new(directive))
        .init();
}

/// Get peers from `SAFE_PEERS` environment variable, first from runtime, then compile-time.
/// If no peers are found and `local` is not enabled, this will panic. Otherwise, it will return an empty list.
#[allow(dead_code)]
pub fn peers_from_run_or_compile_time_env(
) -> Result<Vec<libp2p::Multiaddr>, libp2p::multiaddr::Error> {
    let peers_str = env::var("SAFE_PEERS")
        .ok()
        .or_else(|| option_env!("SAFE_PEERS").map(|s| s.to_string()));

    let Some(peers_str) = peers_str else {
        #[cfg(not(feature = "local-discovery"))]
        panic!("SAFE_PEERS environment variable not set and `local` feature is not enabled");
        #[cfg(feature = "local-discovery")]
        return Ok(vec![]);
    };

    peers_str.split(',').map(parse_peer_addr).collect()
}

/// Parse the `SAFE_PEERS` env var into a list of Multiaddrs.
///
/// An empty `Vec` will be returned if the env var is not set.
#[allow(dead_code)]
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
