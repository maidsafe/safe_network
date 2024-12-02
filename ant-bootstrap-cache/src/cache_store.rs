// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    craft_valid_multiaddr, BootstrapConfig, BootstrapPeer, Error, InitialPeerDiscovery, Result,
};
use fs2::FileExt;
use libp2p::Multiaddr;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};
use tempfile::NamedTempFile;

const PEER_EXPIRY_DURATION: Duration = Duration::from_secs(24 * 60 * 60); // 24 hours

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheData {
    peers: std::collections::HashMap<String, BootstrapPeer>,
    #[serde(default = "SystemTime::now")]
    last_updated: SystemTime,
    #[serde(default = "default_version")]
    version: u32,
}

impl CacheData {
    /// Sync the self cache with another cache by referencing our old_shared_state.
    /// Since the cache is updated on periodic interval, we cannot just add our state with the shared state on the fs.
    /// This would lead to race conditions, hence th need to store the old shared state and sync it with the new shared state.
    pub fn sync(&mut self, old_shared_state: &CacheData, current_shared_state: &CacheData) {
        for (addr, current_shared_peer_state) in current_shared_state.peers.iter() {
            let old_shared_peer_state = old_shared_state.peers.get(addr);
            // If the peer is in the old state, only update the difference in values
            self.peers
                .entry(addr.clone())
                .and_modify(|p| p.sync(old_shared_peer_state, current_shared_peer_state))
                .or_insert_with(|| current_shared_peer_state.clone());
        }

        self.last_updated = SystemTime::now();
    }

    pub fn cleanup_stale_and_unreliable_peers(&mut self) {
        self.peers.retain(|_, peer| peer.is_reliable());
        let now = SystemTime::now();
        self.peers.retain(|_, peer| {
            if let Ok(duration) = now.duration_since(peer.last_seen) {
                duration < PEER_EXPIRY_DURATION
            } else {
                false
            }
        });
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
            match Self::load_cache_data(&self.cache_path).await {
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
        match discovery.fetch_peers().await {
            Ok(peers) => {
                info!("Successfully fetched {} peers from endpoints", peers.len());
                // Only add up to max_peers from the discovered peers
                for peer in peers.into_iter().take(config.max_peers) {
                    data.peers.insert(peer.addr.to_string(), peer);
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

    async fn load_cache_data(cache_path: &PathBuf) -> Result<CacheData> {
        // Try to open the file with read permissions
        let mut file = match OpenOptions::new().read(true).open(cache_path) {
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

        data.cleanup_stale_and_unreliable_peers();

        Ok(data)
    }

    pub fn get_peers(&self) -> impl Iterator<Item = &BootstrapPeer> {
        self.data.peers.values()
    }

    pub fn peer_count(&self) -> usize {
        self.data.peers.len()
    }

    pub fn get_reliable_peers(&self) -> impl Iterator<Item = &BootstrapPeer> {
        self.data
            .peers
            .values()
            .filter(|peer| peer.success_count > peer.failure_count)
    }

    /// Update the status of a peer in the cache. The peer must be added to the cache first.
    pub fn update_peer_status(&mut self, addr: &Multiaddr, success: bool) {
        if let Some(peer) = self.data.peers.get_mut(&addr.to_string()) {
            peer.update_status(success);
        }
    }

    pub fn add_peer(&mut self, addr: Multiaddr) {
        let Some(addr) = craft_valid_multiaddr(&addr) else {
            return;
        };

        let addr_str = addr.to_string();

        // Check if we already have this peer
        if self.data.peers.contains_key(&addr_str) {
            debug!("Updating existing peer's last_seen {addr_str}");
            if let Some(peer) = self.data.peers.get_mut(&addr_str) {
                peer.last_seen = SystemTime::now();
            }
            return;
        }

        self.try_remove_oldest_peers();

        // Add the new peer
        debug!("Adding new peer {} (under max_peers limit)", addr_str);
        self.data.peers.insert(addr_str, BootstrapPeer::new(addr));
    }

    pub fn remove_peer(&mut self, addr: &str) {
        self.data.peers.remove(addr);
    }

    pub fn cleanup_stale_and_unreliable_peers(&mut self) {
        self.data.cleanup_stale_and_unreliable_peers();
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

        if let Ok(data_from_file) = Self::load_cache_data(&self.cache_path).await {
            self.data.sync(&self.old_shared_state, &data_from_file);
            // Now the synced version is the old_shared_state
        } else {
            warn!("Failed to load cache data from file, overwriting with new data");
        }

        if with_cleanup {
            self.data.cleanup_stale_and_unreliable_peers();
            self.try_remove_oldest_peers();
        }
        self.old_shared_state = self.data.clone();

        self.atomic_write().await.inspect_err(|e| {
            error!("Failed to save cache to disk: {e}");
        })
    }

    /// Remove the oldest peers until we're under the max_peers limit
    fn try_remove_oldest_peers(&mut self) {
        // If we're at max peers, remove the oldest peer
        while self.data.peers.len() >= self.config.max_peers {
            if let Some((oldest_addr, _)) = self
                .data
                .peers
                .iter()
                .min_by_key(|(_, peer)| peer.last_seen)
            {
                let oldest_addr = oldest_addr.clone();
                debug!(
                    "At max peers limit ({}), removing oldest peer: {oldest_addr}",
                    self.config.max_peers
                );
                self.data.peers.remove(&oldest_addr);
            }
        }
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
        let addr: Multiaddr = "/ip4/127.0.0.1/tcp/8080".parse().unwrap();

        // Manually add a peer without using fallback
        {
            store
                .data
                .peers
                .insert(addr.to_string(), BootstrapPeer::new(addr.clone()));
        }
        store.sync_and_save_to_disk(true).await.unwrap();

        store.update_peer_status(&addr, true);

        let peers = store.get_peers().collect::<Vec<_>>();
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].addr, addr);
        assert_eq!(peers[0].success_count, 1);
        assert_eq!(peers[0].failure_count, 0);
    }

    #[tokio::test]
    async fn test_peer_cleanup() {
        let (mut store, _) = create_test_store().await;
        let good_addr: Multiaddr = "/ip4/127.0.0.1/tcp/8080".parse().unwrap();
        let bad_addr: Multiaddr = "/ip4/127.0.0.1/tcp/8081".parse().unwrap();

        // Add peers
        store.add_peer(good_addr.clone());
        store.add_peer(bad_addr.clone());

        // Make one peer reliable and one unreliable
        store.update_peer_status(&good_addr, true);

        // Fail the bad peer more times than max_retries
        for _ in 0..5 {
            store.update_peer_status(&bad_addr, false);
        }

        // Clean up unreliable peers
        store.cleanup_stale_and_unreliable_peers();

        // Get all peers (not just reliable ones)
        let peers = store.get_peers().collect::<Vec<_>>();
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].addr, good_addr);
    }

    #[tokio::test]
    async fn test_peer_not_removed_if_successful() {
        let (mut store, _) = create_test_store().await;
        let addr: Multiaddr = "/ip4/127.0.0.1/tcp/8080".parse().unwrap();

        // Add a peer and make it successful
        store.add_peer(addr.clone());
        store.update_peer_status(&addr, true);

        // Wait a bit
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Run cleanup
        store.cleanup_stale_and_unreliable_peers();

        // Verify peer is still there
        let peers = store.get_peers().collect::<Vec<_>>();
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].addr, addr);
    }

    #[tokio::test]
    async fn test_peer_removed_only_when_unresponsive() {
        let (mut store, _) = create_test_store().await;
        let addr: Multiaddr = "/ip4/127.0.0.1/tcp/8080".parse().unwrap();

        // Add a peer
        store.add_peer(addr.clone());

        // Make it fail more than successes
        for _ in 0..3 {
            store.update_peer_status(&addr, true);
        }
        for _ in 0..4 {
            store.update_peer_status(&addr, false);
        }

        // Run cleanup
        store.cleanup_stale_and_unreliable_peers();

        // Verify peer is removed
        assert_eq!(
            store.get_peers().count(),
            0,
            "Peer should be removed after max_retries failures"
        );

        // Test with some successes but more failures
        store.add_peer(addr.clone());
        store.update_peer_status(&addr, true);
        store.update_peer_status(&addr, true);

        for _ in 0..5 {
            store.update_peer_status(&addr, false);
        }

        // Run cleanup
        store.cleanup_stale_and_unreliable_peers();

        // Verify peer is removed due to more failures than successes
        assert_eq!(
            store.get_peers().count(),
            0,
            "Peer should be removed when failures exceed successes"
        );
    }
}
