// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

//! Bootstrap Cache for the Autonomous Network
//!
//! This crate provides a decentralized peer discovery and caching system for the Autonomi Network.
//! It implements a robust peer management system with the following features:
//!
//! - Decentralized Design: No dedicated bootstrap nodes required
//! - Cross-Platform Support: Works on Linux, macOS, and Windows
//! - Shared Cache: System-wide cache file accessible by both nodes and clients
//! - Concurrent Access: File locking for safe multi-process access
//! - Atomic Operations: Safe cache updates using atomic file operations
//! - Initial Peer Discovery: Fallback web endpoints for new/stale cache scenarios
//!
//! # Example
//!
//! ```no_run
//! use bootstrap_cache::{CacheStore, BootstrapConfig, PeersArgs};
//! use url::Url;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = BootstrapConfig::new().unwrap();
//! let args = PeersArgs {
//!     first: false,
//!     peers: vec![],
//!     network_contacts_url: Some(Url::parse("https://example.com/peers")?),
//!     local: false,
//! };
//!
//! let store = CacheStore::from_args(args, config).await?;
//! let peers = store.get_peers().await;
//! # Ok(())
//! # }
//! ```

#[macro_use]
extern crate tracing;

mod cache_store;
pub mod config;
mod error;
mod initial_peer_discovery;

use libp2p::{multiaddr::Protocol, Multiaddr};
use serde::{Deserialize, Serialize};
use std::{fmt, time::SystemTime};
use thiserror::Error;
use url::Url;

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
            self.success_count = self.success_count.saturating_add(1);
        } else {
            self.failure_count = self.failure_count.saturating_add(1);
        }
        self.last_seen = SystemTime::now();
    }

    pub fn is_reliable(&self) -> bool {
        // A peer is considered reliable if it has more successes than failures
        self.success_count >= self.failure_count
    }

    /// If the peer has a old state, just update the difference in values
    /// If the peer has no old state, add the values
    pub fn sync(&mut self, old_shared_state: Option<&Self>, current_shared_state: &Self) {
        if let Some(old_shared_state) = old_shared_state {
            let success_difference = self
                .success_count
                .saturating_sub(old_shared_state.success_count);

            self.success_count = current_shared_state
                .success_count
                .saturating_add(success_difference);

            let failure_difference = self
                .failure_count
                .saturating_sub(old_shared_state.failure_count);
            self.failure_count = current_shared_state
                .failure_count
                .saturating_add(failure_difference);
        } else {
            self.success_count = self
                .success_count
                .saturating_add(current_shared_state.success_count);
            self.failure_count = self
                .failure_count
                .saturating_add(current_shared_state.failure_count);
        }
        self.last_seen = std::cmp::max(self.last_seen, current_shared_state.last_seen);
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

/// Command line arguments for peer configuration
#[derive(Debug, Clone, Default)]
pub struct PeersArgs {
    /// First node in the network
    pub first: bool,
    /// List of peer addresses
    pub peers: Vec<Multiaddr>,
    /// URL to fetch network contacts from
    pub network_contacts_url: Option<Url>,
    /// Use only local discovery (mDNS)
    pub local: bool,
}

impl CacheStore {
    /// Create a new CacheStore from command line arguments
    /// This also initializes the store with the provided peers
    pub async fn from_args(args: PeersArgs, mut config: BootstrapConfig) -> Result<Self> {
        if let Some(url) = &args.network_contacts_url {
            config.endpoints.push(url.clone());
        }

        // If this is the first node, return empty store with no fallback
        if args.first {
            info!("First node in network, returning empty store");
            let store = Self::new_without_init(config).await?;
            store.clear_peers_and_save().await?;
            return Ok(store);
        }

        // If local mode is enabled, return empty store (will use mDNS)
        if args.local {
            info!("Local mode enabled, using only local discovery. Cache writing is disabled");
            config.disable_cache_writing = true;
            let store = Self::new_without_init(config).await?;
            return Ok(store);
        }

        // Create a new store but don't load from cache or fetch from endpoints yet
        let store = Self::new_without_init(config).await?;

        // Add peers from environment variable if present
        if let Ok(env_peers) = std::env::var("SAFE_PEERS") {
            for peer_str in env_peers.split(',') {
                if let Ok(peer) = peer_str.parse() {
                    if let Some(peer) = craft_valid_multiaddr(&peer) {
                        info!("Adding peer from environment: {}", peer);
                        store.add_peer(peer).await;
                    } else {
                        warn!("Invalid peer address format from environment: {}", peer);
                    }
                }
            }
        }

        // Add peers from arguments if present
        for peer in args.peers {
            if let Some(peer) = craft_valid_multiaddr(&peer) {
                info!("Adding peer from arguments: {}", peer);
                store.add_peer(peer).await;
            } else {
                warn!("Invalid peer address format from arguments: {}", peer);
            }
        }

        // If we have a network contacts URL, fetch peers from there.
        if let Some(url) = args.network_contacts_url {
            info!("Fetching peers from network contacts URL: {}", url);
            let peer_discovery = InitialPeerDiscovery::with_endpoints(vec![url])?;
            let peers = peer_discovery.fetch_peers().await?;
            for peer in peers {
                store.add_peer(peer.addr).await;
            }
        }

        // If we have peers, update cache and return, else initialize from cache
        if store.peer_count().await > 0 {
            info!("Using provided peers and updating cache");
            store.sync_to_disk().await?;
        } else {
            store.init().await?;
        }

        Ok(store)
    }
}

/// Craft a proper address to avoid any ill formed addresses
pub fn craft_valid_multiaddr(addr: &Multiaddr) -> Option<Multiaddr> {
    let mut output_address = Multiaddr::empty();

    let ip = addr
        .iter()
        .find(|protocol| matches!(protocol, Protocol::Ip4(_)))?;
    output_address.push(ip);

    let udp = addr
        .iter()
        .find(|protocol| matches!(protocol, Protocol::Udp(_)));
    let tcp = addr
        .iter()
        .find(|protocol| matches!(protocol, Protocol::Tcp(_)));

    // UDP or TCP
    if let Some(udp) = udp {
        output_address.push(udp);
        if let Some(quic) = addr
            .iter()
            .find(|protocol| matches!(protocol, Protocol::QuicV1))
        {
            output_address.push(quic);
        }
    } else if let Some(tcp) = tcp {
        output_address.push(tcp);

        if let Some(ws) = addr
            .iter()
            .find(|protocol| matches!(protocol, Protocol::Ws(_)))
        {
            output_address.push(ws);
        }
    } else {
        return None;
    }

    if let Some(peer_id) = addr
        .iter()
        .find(|protocol| matches!(protocol, Protocol::P2p(_)))
    {
        output_address.push(peer_id);
    }

    Some(output_address)
}

pub fn craft_valid_multiaddr_from_str(addr_str: &str) -> Option<Multiaddr> {
    let Ok(addr) = addr_str.parse::<Multiaddr>() else {
        warn!("Failed to parse multiaddr from str {addr_str}");
        return None;
    };
    craft_valid_multiaddr(&addr)
}
