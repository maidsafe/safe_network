use crate::{
    config::NetworkConfig,
    network::{
        error::NetworkError,
        types::{NetworkDistance, NetworkTimeout},
    },
    types::NodeIssue,
};
use libp2p::PeerId;
use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};
use tracing::{debug, info, warn};

/// Maximum number of failed attempts before a peer is banned
const MAX_FAILED_ATTEMPTS: u32 = 3;
/// Duration for which a peer remains banned
const BAN_DURATION: Duration = Duration::from_secs(300); // 5 minutes

/// Represents the state of a peer connection
#[derive(Debug, Clone)]
struct PeerState {
    /// When the peer was last seen
    last_seen: Instant,
    /// Number of consecutive failed connection attempts
    failed_attempts: u32,
    /// When the peer was banned (if applicable)
    banned_until: Option<Instant>,
    /// Distance from our node
    distance: NetworkDistance,
    /// Set of capabilities supported by this peer
    capabilities: HashSet<String>,
}

/// Manages peer connections and state
#[derive(Debug)]
pub struct PeerManager {
    /// Configuration for the network
    config: NetworkConfig,
    /// Map of peer states
    peers: HashMap<PeerId, PeerState>,
    /// Set of currently connected peers
    connected_peers: HashSet<PeerId>,
    /// Close group of peers (nearest to us)
    close_group: HashSet<PeerId>,
    /// Connection timeout
    connection_timeout: NetworkTimeout,
}

impl PeerManager {
    /// Creates a new PeerManager
    pub fn new(config: NetworkConfig) -> Result<Self, NetworkError> {
        Ok(Self {
            config: config.clone(),
            peers: HashMap::new(),
            connected_peers: HashSet::new(),
            close_group: HashSet::with_capacity(config.close_group_size),
            connection_timeout: NetworkTimeout::new(config.connection_keep_alive)?,
        })
    }

    /// Adds a peer to the manager
    pub fn add_peer(&mut self, peer_id: PeerId) {
        let distance = NetworkDistance::new(0).unwrap(); // TODO: Calculate actual distance
        let state = PeerState {
            last_seen: Instant::now(),
            failed_attempts: 0,
            banned_until: None,
            distance,
            capabilities: HashSet::new(),
        };

        self.peers.insert(peer_id, state);
        self.connected_peers.insert(peer_id);
        self.update_close_group();

        debug!("Added peer {}", peer_id);
    }

    /// Removes a peer from the manager
    pub fn remove_peer(&mut self, peer_id: PeerId) {
        self.connected_peers.remove(&peer_id);
        self.close_group.remove(&peer_id);
        self.update_close_group();

        debug!("Removed peer {}", peer_id);
    }

    /// Records a failed connection attempt for a peer
    pub fn record_failed_attempt(&mut self, peer_id: PeerId, reason: &str) {
        if let Some(state) = self.peers.get_mut(&peer_id) {
            state.failed_attempts += 1;
            if state.failed_attempts >= MAX_FAILED_ATTEMPTS {
                state.banned_until = Some(Instant::now() + BAN_DURATION);
                warn!("Peer {} banned for {} seconds: {}", peer_id, BAN_DURATION.as_secs(), reason);
            }
        }
    }

    /// Checks if a peer is currently banned
    pub fn is_banned(&self, peer_id: &PeerId) -> bool {
        self.peers
            .get(peer_id)
            .and_then(|state| state.banned_until)
            .map(|until| until > Instant::now())
            .unwrap_or(false)
    }

    /// Updates the peer's last seen timestamp
    pub fn update_last_seen(&mut self, peer_id: PeerId) {
        if let Some(state) = self.peers.get_mut(&peer_id) {
            state.last_seen = Instant::now();
            state.failed_attempts = 0; // Reset failed attempts on successful contact
        }
    }

    /// Updates the peer's capabilities
    pub fn update_capabilities(&mut self, peer_id: PeerId, capabilities: HashSet<String>) {
        if let Some(state) = self.peers.get_mut(&peer_id) {
            state.capabilities = capabilities;
        }
    }

    /// Returns the number of connected peers
    pub fn connected_peer_count(&self) -> usize {
        self.connected_peers.len()
    }

    /// Returns the current close group size
    pub fn close_group_size(&self) -> usize {
        self.close_group.len()
    }

    /// Checks if a peer is currently connected
    pub fn is_connected(&self, peer_id: &PeerId) -> bool {
        self.connected_peers.contains(peer_id)
    }

    /// Returns peers that support a specific capability
    pub fn peers_with_capability(&self, capability: &str) -> Vec<PeerId> {
        self.peers
            .iter()
            .filter(|(_, state)| state.capabilities.contains(capability))
            .map(|(peer_id, _)| *peer_id)
            .collect()
    }

    /// Removes expired bans and disconnected peers
    pub fn cleanup(&mut self) {
        let now = Instant::now();
        let timeout = self.connection_timeout.duration();

        self.peers.retain(|peer_id, state| {
            let keep = if let Some(banned_until) = state.banned_until {
                if banned_until <= now {
                    state.banned_until = None;
                    state.failed_attempts = 0;
                    true
                } else {
                    true
                }
            } else if !self.connected_peers.contains(peer_id) 
                && now.duration_since(state.last_seen) > timeout {
                false
            } else {
                true
            };

            if !keep {
                debug!("Cleaned up peer {}", peer_id);
            }
            keep
        });
    }

    /// Updates the close group based on peer distances
    fn update_close_group(&mut self) {
        let mut peers: Vec<_> = self.peers
            .iter()
            .filter(|(peer_id, _)| self.connected_peers.contains(peer_id))
            .collect();

        // Sort by distance
        peers.sort_by_key(|(_, state)| state.distance);

        // Update close group
        self.close_group.clear();
        for (peer_id, _) in peers.iter().take(self.config.close_group_size) {
            self.close_group.insert(**peer_id);
        }

        debug!("Updated close group, size: {}", self.close_group.len());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_peer_management() {
        let config = NetworkConfig::default();
        let mut manager = PeerManager::new(config).unwrap();
        let peer_id = PeerId::random();

        // Test adding peer
        manager.add_peer(peer_id);
        assert!(manager.is_connected(&peer_id));
        assert_eq!(manager.connected_peer_count(), 1);

        // Test removing peer
        manager.remove_peer(peer_id);
        assert!(!manager.is_connected(&peer_id));
        assert_eq!(manager.connected_peer_count(), 0);
    }

    #[test]
    fn test_peer_banning() {
        let config = NetworkConfig::default();
        let mut manager = PeerManager::new(config).unwrap();
        let peer_id = PeerId::random();

        manager.add_peer(peer_id);

        // Test failed attempts
        for _ in 0..MAX_FAILED_ATTEMPTS {
            manager.record_failed_attempt(peer_id, "test failure");
        }

        assert!(manager.is_banned(&peer_id));
    }

    #[test]
    fn test_peer_capabilities() {
        let config = NetworkConfig::default();
        let mut manager = PeerManager::new(config).unwrap();
        let peer_id = PeerId::random();

        manager.add_peer(peer_id);

        let mut capabilities = HashSet::new();
        capabilities.insert("relay".to_string());
        capabilities.insert("store".to_string());

        manager.update_capabilities(peer_id, capabilities);

        assert_eq!(manager.peers_with_capability("relay").len(), 1);
        assert_eq!(manager.peers_with_capability("unknown").len(), 0);
    }

    #[test]
    fn test_close_group_updates() {
        let config = NetworkConfig::default();
        let mut manager = PeerManager::new(config.clone()).unwrap();

        // Add multiple peers
        for _ in 0..config.close_group_size + 2 {
            manager.add_peer(PeerId::random());
        }

        assert!(manager.close_group_size() <= config.close_group_size);
    }

    #[test]
    fn test_peer_cleanup() {
        let config = NetworkConfig::default();
        let mut manager = PeerManager::new(config).unwrap();
        let peer_id = PeerId::random();

        manager.add_peer(peer_id);
        manager.remove_peer(peer_id);

        // Simulate time passing
        std::thread::sleep(Duration::from_millis(100));

        manager.cleanup();
        assert!(!manager.peers.contains_key(&peer_id));
    }
} 