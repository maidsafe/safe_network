// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    craft_valid_multiaddr, multiaddr_get_peer_id, BootstrapAddr, BootstrapAddresses,
    BootstrapConfig, Error, InitialPeerDiscovery, Result,
};
use fs2::FileExt;
use libp2p::multiaddr::Protocol;
use libp2p::{Multiaddr, PeerId};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};
use tempfile::NamedTempFile;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheData {
    peers: std::collections::HashMap<PeerId, BootstrapAddresses>,
    #[serde(default = "SystemTime::now")]
    last_updated: SystemTime,
    #[serde(default = "default_version")]
    version: u32,
}

impl CacheData {
    pub fn insert(&mut self, peer_id: PeerId, bootstrap_addr: BootstrapAddr) {
        match self.peers.entry(peer_id) {
            Entry::Occupied(mut occupied_entry) => {
                occupied_entry.get_mut().insert_addr(&bootstrap_addr);
            }
            Entry::Vacant(vacant_entry) => {
                vacant_entry.insert(BootstrapAddresses(vec![bootstrap_addr]));
            }
        }
    }

    /// Sync the self cache with another cache by referencing our old_shared_state.
    /// Since the cache is updated on periodic interval, we cannot just add our state with the shared state on the fs.
    /// This would lead to race conditions, hence the need to store the old shared state in memory and sync it with the
    /// new shared state obtained from fs.
    pub fn sync(&mut self, old_shared_state: &CacheData, current_shared_state: &CacheData) {
        // Add/sync every BootstrapAddresses from shared state into self
        for (peer, current_shared_addrs_state) in current_shared_state.peers.iter() {
            let old_shared_addrs_state = old_shared_state.peers.get(peer);
            let bootstrap_addresses = self
                .peers
                .entry(*peer)
                .or_insert(current_shared_addrs_state.clone());

            // Add/sync every BootstrapAddr into self
            bootstrap_addresses.sync(old_shared_addrs_state, current_shared_addrs_state);
        }

        self.last_updated = SystemTime::now();
    }

    /// Perform cleanup on the Peers
    /// - Removes all the unreliable addrs for a peer
    /// - Removes all the expired addrs for a peer
    /// - Removes all peers with empty addrs set
    /// - Maintains `max_addr` per peer by removing the addr with the lowest success rate
    /// - Maintains `max_peers` in the list by removing the peer with the oldest last_seen
    pub fn perform_cleanup(&mut self, cfg: &BootstrapConfig) {
        self.peers.values_mut().for_each(|bootstrap_addresses| {
            bootstrap_addresses.0.retain(|bootstrap_addr| {
                let now = SystemTime::now();
                let has_not_expired =
                    if let Ok(duration) = now.duration_since(bootstrap_addr.last_seen) {
                        duration < cfg.addr_expiry_duration
                    } else {
                        false
                    };
                bootstrap_addr.is_reliable() && has_not_expired
            })
        });

        self.peers
            .retain(|_, bootstrap_addresses| !bootstrap_addresses.0.is_empty());

        self.peers.values_mut().for_each(|bootstrap_addresses| {
            if bootstrap_addresses.0.len() > cfg.max_addrs_per_peer {
                // sort by lowest failure rate first
                bootstrap_addresses
                    .0
                    .sort_by_key(|addr| addr.failure_rate() as u64);
                bootstrap_addresses.0.truncate(cfg.max_addrs_per_peer);
            }
        });

        self.try_remove_oldest_peers(cfg);
    }

    /// Remove the oldest peers until we're under the max_peers limit
    pub fn try_remove_oldest_peers(&mut self, cfg: &BootstrapConfig) {
        if self.peers.len() > cfg.max_peers {
            let mut peer_last_seen_map = HashMap::new();
            for (peer, addrs) in self.peers.iter() {
                let mut latest_seen = Duration::from_secs(u64::MAX);
                for addr in addrs.0.iter() {
                    if let Ok(elapsed) = addr.last_seen.elapsed() {
                        trace!("Time elapsed for {addr:?} is {elapsed:?}");
                        if elapsed < latest_seen {
                            trace!("Updating latest_seen to {elapsed:?}");
                            latest_seen = elapsed;
                        }
                    }
                }
                trace!("Last seen for {peer:?} is {latest_seen:?}");
                peer_last_seen_map.insert(*peer, latest_seen);
            }

            while self.peers.len() > cfg.max_peers {
                // find the peer with the largest last_seen
                if let Some((&oldest_peer, last_seen)) = peer_last_seen_map
                    .iter()
                    .max_by_key(|(_, last_seen)| **last_seen)
                {
                    debug!("Found the oldest peer to remove: {oldest_peer:?} with last_seen of {last_seen:?}");
                    self.peers.remove(&oldest_peer);
                    peer_last_seen_map.remove(&oldest_peer);
                }
            }
        }
    }
}

fn default_version() -> u32 {
    1
}

impl Default for CacheData {
    fn default() -> Self {
        Self {
            peers: std::collections::HashMap::new(),
            last_updated: SystemTime::now(),
            version: default_version(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct BootstrapCacheStore {
    cache_path: PathBuf,
    config: BootstrapConfig,
    data: CacheData,
    /// This is our last known state of the cache on disk, which is shared across all instances.
    /// This is not updated until `sync_to_disk` is called.
    old_shared_state: CacheData,
}

impl BootstrapCacheStore {
    pub fn config(&self) -> &BootstrapConfig {
        &self.config
    }

    pub async fn new(config: BootstrapConfig) -> Result<Self> {
        info!("Creating new CacheStore with config: {:?}", config);
        let cache_path = config.cache_file_path.clone();

        // Create cache directory if it doesn't exist
        if let Some(parent) = cache_path.parent() {
            if !parent.exists() {
                info!("Attempting to create cache directory at {parent:?}");
                fs::create_dir_all(parent).inspect_err(|err| {
                    warn!("Failed to create cache directory at {parent:?}: {err}");
                })?;
            }
        }

        let mut store = Self {
            cache_path,
            config,
            data: CacheData::default(),
            old_shared_state: CacheData::default(),
        };

        store.init().await?;

        info!("Successfully created CacheStore and initialized it.");

        Ok(store)
    }

    pub async fn new_without_init(config: BootstrapConfig) -> Result<Self> {
        info!("Creating new CacheStore with config: {:?}", config);
        let cache_path = config.cache_file_path.clone();

        // Create cache directory if it doesn't exist
        if let Some(parent) = cache_path.parent() {
            if !parent.exists() {
                info!("Attempting to create cache directory at {parent:?}");
                fs::create_dir_all(parent).inspect_err(|err| {
                    warn!("Failed to create cache directory at {parent:?}: {err}");
                })?;
            }
        }

        let store = Self {
            cache_path,
            config,
            data: CacheData::default(),
            old_shared_state: CacheData::default(),
        };

        info!("Successfully created CacheStore without initializing the data.");
        Ok(store)
    }

    pub async fn init(&mut self) -> Result<()> {
        let data = if self.cache_path.exists() {
            info!(
                "Cache file exists at {:?}, attempting to load",
                self.cache_path
            );
            match Self::load_cache_data(&self.config).await {
                Ok(data) => {
                    info!(
                        "Successfully loaded cache data with {} peers",
                        data.peers.len()
                    );
                    // If cache data exists but has no peers and file is not read-only,
                    // fallback to default
                    let is_readonly = self
                        .cache_path
                        .metadata()
                        .map(|m| m.permissions().readonly())
                        .unwrap_or(false);

                    if data.peers.is_empty() && !is_readonly {
                        info!("Cache is empty and not read-only, falling back to default");
                        Self::fallback_to_default(&self.config).await?
                    } else {
                        // Ensure we don't exceed max_peers
                        let mut filtered_data = data;
                        if filtered_data.peers.len() > self.config.max_peers {
                            info!(
                                "Trimming cache from {} to {} peers",
                                filtered_data.peers.len(),
                                self.config.max_peers
                            );

                            filtered_data.peers = filtered_data
                                .peers
                                .into_iter()
                                .take(self.config.max_peers)
                                .collect();
                        }
                        filtered_data
                    }
                }
                Err(e) => {
                    warn!("Failed to load cache data: {}", e);
                    // If we can't read or parse the cache file, fallback to default
                    Self::fallback_to_default(&self.config).await?
                }
            }
        } else {
            info!(
                "Cache file does not exist at {:?}, falling back to default",
                self.cache_path
            );
            // If cache file doesn't exist, fallback to default
            Self::fallback_to_default(&self.config).await?
        };

        // Update the store's data
        self.data = data.clone();
        self.old_shared_state = data;

        // Save the default data to disk
        self.sync_and_save_to_disk(false).await?;

        Ok(())
    }

    async fn fallback_to_default(config: &BootstrapConfig) -> Result<CacheData> {
        info!("Falling back to default peers from endpoints");
        let mut data = CacheData {
            peers: std::collections::HashMap::new(),
            last_updated: SystemTime::now(),
            version: default_version(),
        };

        // If no endpoints are configured, just return empty cache
        if config.endpoints.is_empty() {
            warn!("No endpoints configured, returning empty cache");
            return Ok(data);
        }

        // Try to discover peers from configured endpoints
        let discovery = InitialPeerDiscovery::with_endpoints(config.endpoints.clone())?;
        match discovery.fetch_bootstrap_addresses().await {
            Ok(addrs) => {
                info!("Successfully fetched {} peers from endpoints", addrs.len());
                // Only add up to max_peers from the discovered peers
                let mut count = 0;
                for bootstrap_addr in addrs.into_iter() {
                    if count >= config.max_peers {
                        break;
                    }
                    if let Some(peer_id) = bootstrap_addr.peer_id() {
                        data.insert(peer_id, bootstrap_addr);
                        count += 1;
                    }
                }

                // Create parent directory if it doesn't exist
                if let Some(parent) = config.cache_file_path.parent() {
                    if !parent.exists() {
                        info!("Creating cache directory at {:?}", parent);
                        if let Err(e) = fs::create_dir_all(parent) {
                            warn!("Failed to create cache directory: {}", e);
                        }
                    }
                }

                // Try to write the cache file immediately
                match serde_json::to_string_pretty(&data) {
                    Ok(json) => {
                        info!("Writing {} peers to cache file", data.peers.len());
                        if let Err(e) = fs::write(&config.cache_file_path, json) {
                            warn!("Failed to write cache file: {}", e);
                        } else {
                            info!(
                                "Successfully wrote cache file at {:?}",
                                config.cache_file_path
                            );
                        }
                    }
                    Err(e) => {
                        warn!("Failed to serialize cache data: {}", e);
                    }
                }

                Ok(data)
            }
            Err(e) => {
                warn!("Failed to fetch peers from endpoints: {}", e);
                Ok(data) // Return empty cache on error
            }
        }
    }

    async fn load_cache_data(cfg: &BootstrapConfig) -> Result<CacheData> {
        // Try to open the file with read permissions
        let mut file = match OpenOptions::new().read(true).open(&cfg.cache_file_path) {
            Ok(f) => f,
            Err(e) => {
                warn!("Failed to open cache file: {}", e);
                return Err(Error::from(e));
            }
        };

        // Acquire shared lock for reading
        if let Err(e) = Self::acquire_shared_lock(&file).await {
            warn!("Failed to acquire shared lock: {}", e);
            return Err(e);
        }

        // Read the file contents
        let mut contents = String::new();
        if let Err(e) = file.read_to_string(&mut contents) {
            warn!("Failed to read cache file: {}", e);
            return Err(Error::from(e));
        }

        // Parse the cache data
        let mut data = serde_json::from_str::<CacheData>(&contents).map_err(|e| {
            warn!("Failed to parse cache data: {}", e);
            Error::FailedToParseCacheData
        })?;

        data.perform_cleanup(cfg);

        Ok(data)
    }

    pub fn peer_count(&self) -> usize {
        self.data.peers.len()
    }

    pub fn get_addrs(&self) -> impl Iterator<Item = &BootstrapAddr> {
        self.data
            .peers
            .values()
            .flat_map(|bootstrap_addresses| bootstrap_addresses.0.iter())
    }

    pub fn get_reliable_addrs(&self) -> impl Iterator<Item = &BootstrapAddr> {
        self.data
            .peers
            .values()
            .flat_map(|bootstrap_addresses| bootstrap_addresses.0.iter())
            .filter(|bootstrap_addr| bootstrap_addr.is_reliable())
    }

    /// Update the status of an addr in the cache. The peer must be added to the cache first.
    pub fn update_addr_status(&mut self, addr: &Multiaddr, success: bool) {
        if let Some(peer_id) = multiaddr_get_peer_id(addr) {
            debug!("Updating addr status: {addr} (success: {success})");
            if let Some(bootstrap_addresses) = self.data.peers.get_mut(&peer_id) {
                bootstrap_addresses.update_addr_status(addr, success);
            } else {
                debug!("Peer not found in cache to update: {addr}");
            }
        }
    }

    /// Add a set of addresses to the cache.
    pub fn add_addr(&mut self, addr: Multiaddr) {
        debug!("Trying to add new addr: {addr}");
        let Some(addr) = craft_valid_multiaddr(&addr) else {
            return;
        };
        let peer_id = match addr.iter().find(|p| matches!(p, Protocol::P2p(_))) {
            Some(Protocol::P2p(id)) => id,
            _ => return,
        };

        // Check if we already have this peer
        if let Some(bootstrap_addrs) = self.data.peers.get_mut(&peer_id) {
            if let Some(bootstrap_addr) = bootstrap_addrs.get_addr_mut(&addr) {
                debug!("Updating existing peer's last_seen {addr}");
                bootstrap_addr.last_seen = SystemTime::now();
                return;
            } else {
                bootstrap_addrs.insert_addr(&BootstrapAddr::new(addr.clone()));
            }
        } else {
            self.data.peers.insert(
                peer_id,
                BootstrapAddresses(vec![BootstrapAddr::new(addr.clone())]),
            );
        }

        debug!("Added new peer {addr:?}, performing cleanup of old addrs");
        self.perform_cleanup();
    }

    /// Remove a single address for a peer.
    pub fn remove_addr(&mut self, addr: &Multiaddr) {
        if let Some(peer_id) = multiaddr_get_peer_id(addr) {
            if let Some(bootstrap_addresses) = self.data.peers.get_mut(&peer_id) {
                bootstrap_addresses.remove_addr(addr);
            } else {
                debug!("Peer {peer_id:?} not found in the cache. Not removing addr: {addr:?}")
            }
        } else {
            debug!("Could not obtain PeerId for {addr:?}, not removing addr from cache.");
        }
    }

    pub fn perform_cleanup(&mut self) {
        self.data.perform_cleanup(&self.config);
    }

    /// Clear all peers from the cache and save to disk
    pub async fn clear_peers_and_save(&mut self) -> Result<()> {
        self.data.peers.clear();
        self.old_shared_state.peers.clear();

        match self.atomic_write().await {
            Ok(_) => Ok(()),
            Err(e) => {
                error!("Failed to save cache to disk: {e}");
                Err(e)
            }
        }
    }

    /// Do not perform cleanup when `data` is fetched from the network.
    /// The SystemTime might not be accurate.
    pub async fn sync_and_save_to_disk(&mut self, with_cleanup: bool) -> Result<()> {
        if self.config.disable_cache_writing {
            info!("Cache writing is disabled, skipping sync to disk");
            return Ok(());
        }

        info!(
            "Syncing cache to disk, with data containing: {} peers and old state containing: {} peers", self.data.peers.len(),
            self.old_shared_state.peers.len()
        );

        // Check if the file is read-only before attempting to write
        let is_readonly = self
            .cache_path
            .metadata()
            .map(|m| m.permissions().readonly())
            .unwrap_or(false);

        if is_readonly {
            warn!("Cannot save to disk: cache file is read-only");
            // todo return err
            return Ok(());
        }

        if let Ok(data_from_file) = Self::load_cache_data(&self.config).await {
            self.data.sync(&self.old_shared_state, &data_from_file);
            // Now the synced version is the old_shared_state
        } else {
            warn!("Failed to load cache data from file, overwriting with new data");
        }

        if with_cleanup {
            self.data.perform_cleanup(&self.config);
            self.data.try_remove_oldest_peers(&self.config);
        }
        self.old_shared_state = self.data.clone();

        self.atomic_write().await.inspect_err(|e| {
            error!("Failed to save cache to disk: {e}");
        })
    }

    async fn acquire_shared_lock(file: &File) -> Result<()> {
        let file = file.try_clone().map_err(Error::from)?;

        tokio::task::spawn_blocking(move || file.try_lock_shared().map_err(Error::from))
            .await
            .map_err(|e| {
                Error::from(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to spawn blocking task: {}", e),
                ))
            })?
    }

    async fn acquire_exclusive_lock(file: &File) -> Result<()> {
        let mut backoff = Duration::from_millis(10);
        let max_attempts = 5;
        let mut attempts = 0;

        loop {
            match file.try_lock_exclusive() {
                Ok(_) => return Ok(()),
                Err(_) if attempts >= max_attempts => {
                    return Err(Error::LockError);
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    attempts += 1;
                    tokio::time::sleep(backoff).await;
                    backoff *= 2;
                }
                Err(_) => return Err(Error::LockError),
            }
        }
    }

    async fn atomic_write(&self) -> Result<()> {
        // Create parent directory if it doesn't exist
        if let Some(parent) = self.cache_path.parent() {
            fs::create_dir_all(parent).map_err(Error::from)?;
        }

        // Create a temporary file in the same directory as the cache file
        let temp_file = NamedTempFile::new().map_err(Error::from)?;

        // Write data to temporary file
        serde_json::to_writer_pretty(&temp_file, &self.data).map_err(Error::from)?;

        // Open the target file with proper permissions
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&self.cache_path)
            .map_err(Error::from)?;

        // Acquire exclusive lock
        Self::acquire_exclusive_lock(&file).await?;

        // Perform atomic rename
        temp_file.persist(&self.cache_path).inspect_err(|err| {
            error!("Failed to persist file with err: {err:?}");
        })?;

        // Lock will be automatically released when file is dropped
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    async fn create_test_store() -> (BootstrapCacheStore, PathBuf) {
        let temp_dir = tempdir().unwrap();
        let cache_file = temp_dir.path().join("cache.json");

        let config = crate::BootstrapConfig::empty()
            .unwrap()
            .with_cache_path(&cache_file);

        let store = BootstrapCacheStore::new(config).await.unwrap();
        (store.clone(), store.cache_path.clone())
    }

    #[tokio::test]
    async fn test_peer_update_and_save() {
        let (mut store, _) = create_test_store().await;
        let addr: Multiaddr =
            "/ip4/127.0.0.1/tcp/8080/p2p/12D3KooWRBhwfeP2Y4TCx1SM6s9rUoHhR5STiGwxBhgFRcw3UERE"
                .parse()
                .unwrap();

        // Manually add a peer without using fallback
        {
            let peer_id = multiaddr_get_peer_id(&addr).unwrap();
            store.data.insert(peer_id, BootstrapAddr::new(addr.clone()));
        }
        store.sync_and_save_to_disk(true).await.unwrap();

        store.update_addr_status(&addr, true);

        let peers = store.get_addrs().collect::<Vec<_>>();
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].addr, addr);
        assert_eq!(peers[0].success_count, 1);
        assert_eq!(peers[0].failure_count, 0);
    }

    #[tokio::test]
    async fn test_peer_cleanup() {
        let (mut store, _) = create_test_store().await;
        let good_addr: Multiaddr =
            "/ip4/127.0.0.1/tcp/8080/p2p/12D3KooWRBhwfeP2Y4TCx1SM6s9rUoHhR5STiGwxBhgFRcw3UERE"
                .parse()
                .unwrap();
        let bad_addr: Multiaddr =
            "/ip4/127.0.0.1/tcp/8081/p2p/12D3KooWD2aV1f3qkhggzEFaJ24CEFYkSdZF5RKoMLpU6CwExYV5"
                .parse()
                .unwrap();

        // Add peers
        store.add_addr(good_addr.clone());
        store.add_addr(bad_addr.clone());

        // Make one peer reliable and one unreliable
        store.update_addr_status(&good_addr, true);

        // Fail the bad peer more times than max_retries
        for _ in 0..5 {
            store.update_addr_status(&bad_addr, false);
        }

        // Clean up unreliable peers
        store.perform_cleanup();

        // Get all peers (not just reliable ones)
        let peers = store.get_addrs().collect::<Vec<_>>();
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].addr, good_addr);
    }

    #[tokio::test]
    async fn test_peer_not_removed_if_successful() {
        let (mut store, _) = create_test_store().await;
        let addr: Multiaddr =
            "/ip4/127.0.0.1/tcp/8080/p2p/12D3KooWRBhwfeP2Y4TCx1SM6s9rUoHhR5STiGwxBhgFRcw3UERE"
                .parse()
                .unwrap();

        // Add a peer and make it successful
        store.add_addr(addr.clone());
        store.update_addr_status(&addr, true);

        // Wait a bit
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Run cleanup
        store.perform_cleanup();

        // Verify peer is still there
        let peers = store.get_addrs().collect::<Vec<_>>();
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].addr, addr);
    }

    #[tokio::test]
    async fn test_peer_removed_only_when_unresponsive() {
        let (mut store, _) = create_test_store().await;
        let addr: Multiaddr =
            "/ip4/127.0.0.1/tcp/8080/p2p/12D3KooWRBhwfeP2Y4TCx1SM6s9rUoHhR5STiGwxBhgFRcw3UERE"
                .parse()
                .unwrap();

        // Add a peer
        store.add_addr(addr.clone());

        // Make it fail more than successes
        for _ in 0..3 {
            store.update_addr_status(&addr, true);
        }
        for _ in 0..4 {
            store.update_addr_status(&addr, false);
        }

        // Run cleanup
        store.perform_cleanup();

        // Verify peer is removed
        assert_eq!(
            store.get_addrs().count(),
            0,
            "Peer should be removed after max_retries failures"
        );

        // Test with some successes but more failures
        store.add_addr(addr.clone());
        store.update_addr_status(&addr, true);
        store.update_addr_status(&addr, true);

        for _ in 0..5 {
            store.update_addr_status(&addr, false);
        }

        // Run cleanup
        store.perform_cleanup();

        // Verify peer is removed due to more failures than successes
        assert_eq!(
            store.get_addrs().count(),
            0,
            "Peer should be removed when failures exceed successes"
        );
    }
}
