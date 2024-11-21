// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod cache_store;
mod circuit_breaker;
pub mod config;
mod error;
mod initial_peer_discovery;

use libp2p::Multiaddr;
use serde::{Deserialize, Serialize};
use std::{fmt, time::SystemTime};
use thiserror::Error;

pub use cache_store::CacheStore;
pub use config::BootstrapConfig;
pub use error::{Error, Result};
pub use initial_peer_discovery::InitialPeerDiscovery;

/// Structure representing a list of bootstrap endpoints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapEndpoints {
    /// List of peer multiaddresses
    pub peers: Vec<String>,
    /// Optional metadata about the endpoints
    #[serde(default)]
    pub metadata: EndpointMetadata,
}

/// Metadata about bootstrap endpoints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointMetadata {
    /// When the endpoints were last updated
    #[serde(default = "default_last_updated")]
    pub last_updated: String,
    /// Optional description of the endpoints
    #[serde(default)]
    pub description: String,
}

fn default_last_updated() -> String {
    chrono::Utc::now().to_rfc3339()
}

impl Default for EndpointMetadata {
    fn default() -> Self {
        Self {
            last_updated: default_last_updated(),
            description: String::new(),
        }
    }
}

/// A peer that can be used for bootstrapping into the network
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapPeer {
    /// The multiaddress of the peer
    pub addr: Multiaddr,
    /// The number of successful connections to this peer
    pub success_count: u32,
    /// The number of failed connection attempts to this peer
    pub failure_count: u32,
    /// The last time this peer was successfully contacted
    pub last_seen: SystemTime,
}

impl BootstrapPeer {
    pub fn new(addr: Multiaddr) -> Self {
        Self {
            addr,
            success_count: 0,
            failure_count: 0,
            last_seen: SystemTime::now(),
        }
    }

    pub fn update_status(&mut self, success: bool) {
        if success {
            self.success_count += 1;
            self.last_seen = SystemTime::now();
        } else {
            self.failure_count += 1;
        }
    }

    pub fn is_reliable(&self) -> bool {
        // A peer is considered reliable if it has more successes than failures
        self.success_count > self.failure_count
    }
}

impl fmt::Display for BootstrapPeer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "BootstrapPeer {{ addr: {}, last_seen: {:?}, success: {}, failure: {} }}",
            self.addr, self.last_seen, self.success_count, self.failure_count
        )
    }
}

/// Creates a new bootstrap cache with default configuration
pub async fn new() -> Result<CacheStore> {
    CacheStore::new(BootstrapConfig::default()).await
}

/// Creates a new bootstrap cache with custom configuration
pub async fn with_config(config: BootstrapConfig) -> Result<CacheStore> {
    CacheStore::new(config).await
}
