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
//! use ant_bootstrap_cache::{BootstrapCacheStore, BootstrapConfig, PeersArgs};
//! use url::Url;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = BootstrapConfig::empty().unwrap();
//! let args = PeersArgs {
//!     first: false,
//!     addrs: vec![],
//!     network_contacts_url: Some(Url::parse("https://example.com/peers")?),
//!     local: false,
//! };
//!
//! let store = BootstrapCacheStore::from_args(args, config).await?;
//! let addrs = store.get_addrs();
//! # Ok(())
//! # }
//! ```

#[macro_use]
extern crate tracing;

mod cache_store;
pub mod config;
mod error;
mod initial_peer_discovery;

use libp2p::{multiaddr::Protocol, Multiaddr, PeerId};
use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use thiserror::Error;
use url::Url;

pub use cache_store::BootstrapCacheStore;
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

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Set of addresses for a particular PeerId
pub struct BootstrapAddresses(pub Vec<BootstrapAddr>);

impl BootstrapAddresses {
    pub fn insert_addr(&mut self, addr: &BootstrapAddr) {
        if let Some(bootstrap_addr) = self.get_addr_mut(&addr.addr) {
            bootstrap_addr.sync(None, addr);
        } else {
            self.0.push(addr.clone());
        }
    }

    pub fn get_addr(&self, addr: &Multiaddr) -> Option<&BootstrapAddr> {
        self.0
            .iter()
            .find(|bootstrap_addr| &bootstrap_addr.addr == addr)
    }

    pub fn get_addr_mut(&mut self, addr: &Multiaddr) -> Option<&mut BootstrapAddr> {
        self.0
            .iter_mut()
            .find(|bootstrap_addr| &bootstrap_addr.addr == addr)
    }

    pub fn remove_addr(&mut self, addr: &Multiaddr) {
        if let Some(idx) = self
            .0
            .iter()
            .position(|bootstrap_addr| &bootstrap_addr.addr == addr)
        {
            let bootstrap_addr = self.0.remove(idx);
            debug!("Removed {bootstrap_addr:?}");
        }
    }

    pub fn sync(&mut self, old_shared_state: Option<&Self>, current_shared_state: &Self) {
        for current_bootstrap_addr in current_shared_state.0.iter() {
            if let Some(bootstrap_addr) = self.get_addr_mut(&current_bootstrap_addr.addr) {
                let old_bootstrap_addr = old_shared_state.and_then(|old_shared_state| {
                    old_shared_state.get_addr(&current_bootstrap_addr.addr)
                });
                bootstrap_addr.sync(old_bootstrap_addr, current_bootstrap_addr);
            } else {
                self.insert_addr(current_bootstrap_addr);
            }
        }
    }

    pub fn update_addr_status(&mut self, addr: &Multiaddr, success: bool) {
        if let Some(bootstrap_addr) = self.get_addr_mut(addr) {
            bootstrap_addr.update_status(success);
        } else {
            debug!("Addr not found in cache to update, skipping: {addr:?}")
        }
    }
}

/// A addr that can be used for bootstrapping into the network
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapAddr {
    /// The multiaddress of the peer
    pub addr: Multiaddr,
    /// The number of successful connections to this address
    pub success_count: u32,
    /// The number of failed connection attempts to this address
    pub failure_count: u32,
    /// The last time this address was successfully contacted
    pub last_seen: SystemTime,
}

impl BootstrapAddr {
    pub fn new(addr: Multiaddr) -> Self {
        Self {
            addr,
            success_count: 0,
            failure_count: 0,
            last_seen: SystemTime::now(),
        }
    }

    pub fn peer_id(&self) -> Option<PeerId> {
        multiaddr_get_peer_id(&self.addr)
    }

    pub fn update_status(&mut self, success: bool) {
        if success {
            if let Some(new_value) = self.success_count.checked_add(1) {
                self.success_count = new_value;
            } else {
                self.success_count = 1;
                self.failure_count = 0;
            }
        }
        self.last_seen = SystemTime::now();
        if !success {
            if let Some(new_value) = self.failure_count.checked_add(1) {
                self.failure_count = new_value;
            } else {
                self.failure_count = 1;
                self.success_count = 0;
            }
        }
    }

    // An addr is considered reliable if it has more successes than failures
    pub fn is_reliable(&self) -> bool {
        self.success_count >= self.failure_count
    }

    /// If the peer has a old state, just update the difference in values
    /// If the peer has no old state, add the values
    pub fn sync(&mut self, old_shared_state: Option<&Self>, current_shared_state: &Self) {
        if self.last_seen == current_shared_state.last_seen {
            return;
        }

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

        // if at max value, reset to 0
        if self.success_count == u32::MAX {
            self.success_count = 1;
            self.failure_count = 0;
        } else if self.failure_count == u32::MAX {
            self.failure_count = 1;
            self.success_count = 0;
        }

        self.last_seen = std::cmp::max(self.last_seen, current_shared_state.last_seen);
    }

    fn failure_rate(&self) -> f64 {
        if self.success_count + self.failure_count == 0 {
            0.0
        } else {
            self.failure_count as f64 / (self.success_count + self.failure_count) as f64
        }
    }
}

/// Command line arguments for peer configuration
#[derive(Debug, Clone, Default)]
pub struct PeersArgs {
    /// First node in the network
    pub first: bool,
    /// List of addresses
    pub addrs: Vec<Multiaddr>,
    /// URL to fetch network contacts from
    pub network_contacts_url: Option<Url>,
    /// Use only local discovery (mDNS)
    pub local: bool,
}

impl BootstrapCacheStore {
    /// Create a new CacheStore from command line arguments
    /// This also initializes the store with the provided bootstrap addresses
    pub async fn from_args(args: PeersArgs, mut config: BootstrapConfig) -> Result<Self> {
        if let Some(url) = &args.network_contacts_url {
            config.endpoints.push(url.clone());
        }

        // If this is the first node, return empty store with no fallback
        if args.first {
            info!("First node in network, returning empty store");
            let mut store = Self::new_without_init(config).await?;
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
        let mut store = Self::new_without_init(config).await?;

        // Add addrs from environment variable if present
        if let Ok(env_string) = std::env::var("SAFE_PEERS") {
            for multiaddr_str in env_string.split(',') {
                if let Ok(addr) = multiaddr_str.parse() {
                    if let Some(addr) = craft_valid_multiaddr(&addr) {
                        info!("Adding addr from environment: {addr}",);
                        store.add_addr(addr);
                    } else {
                        warn!("Invalid peer address format from environment: {}", addr);
                    }
                }
            }
        }

        // Add addrs from arguments if present
        for addr in args.addrs {
            if let Some(addr) = craft_valid_multiaddr(&addr) {
                info!("Adding addr from arguments: {addr}");
                store.add_addr(addr);
            } else {
                warn!("Invalid multiaddress format from arguments: {addr}");
            }
        }

        // If we have a network contacts URL, fetch addrs from there.
        if let Some(url) = args.network_contacts_url {
            info!(
                "Fetching bootstrap address from network contacts URL: {}",
                url
            );
            let peer_discovery = InitialPeerDiscovery::with_endpoints(vec![url])?;
            let bootstrap_addresses = peer_discovery.fetch_bootstrap_addresses().await?;
            for addr in bootstrap_addresses {
                store.add_addr(addr.addr);
            }
        }

        // If we have peers, update cache and return, else initialize from cache
        if store.peer_count() > 0 {
            info!("Using provided peers and updating cache");
            store.sync_and_save_to_disk(false).await?;
        } else {
            store.init().await?;
        }

        Ok(store)
    }
}

/// Craft a proper address to avoid any ill formed addresses
pub fn craft_valid_multiaddr(addr: &Multiaddr) -> Option<Multiaddr> {
    let peer_id = addr
        .iter()
        .find(|protocol| matches!(protocol, Protocol::P2p(_)))?;

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

    output_address.push(peer_id);

    Some(output_address)
}

pub fn craft_valid_multiaddr_from_str(addr_str: &str) -> Option<Multiaddr> {
    let Ok(addr) = addr_str.parse::<Multiaddr>() else {
        warn!("Failed to parse multiaddr from str {addr_str}");
        return None;
    };
    craft_valid_multiaddr(&addr)
}

pub fn multiaddr_get_peer_id(addr: &Multiaddr) -> Option<PeerId> {
    match addr.iter().find(|p| matches!(p, Protocol::P2p(_))) {
        Some(Protocol::P2p(id)) => Some(id),
        _ => None,
    }
}
