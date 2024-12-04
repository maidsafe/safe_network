use crate::{
    config::NetworkConfig,
    network::{
        error::{NetworkError, RecordError, RecordResult},
        types::NetworkTimeout,
    },
};
use libp2p::{kad::{Record, store::MemoryStore}, PeerId};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, SystemTime},
};
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// Maximum size for a single record in bytes
const MAX_RECORD_SIZE: usize = 1024 * 1024; // 1MB

/// Storage for network records with validation and expiry
#[derive(Debug)]
pub struct RecordStorage {
    /// Internal storage for records
    store: MemoryStore,
    /// Record metadata
    metadata: HashMap<Vec<u8>, RecordMetadata>,
    /// Configuration
    config: NetworkConfig,
    /// Record expiry timeout
    expiry_timeout: NetworkTimeout,
}

/// Metadata for stored records
#[derive(Debug, Clone)]
struct RecordMetadata {
    /// Time when the record was stored
    stored_at: SystemTime,
    /// Publisher of the record
    publisher: Option<PeerId>,
    /// Size of the record in bytes
    size: usize,
    /// Number of times the record has been retrieved
    access_count: u64,
}

impl RecordStorage {
    /// Creates a new RecordStorage instance
    pub fn new(config: NetworkConfig) -> Result<Self, NetworkError> {
        Ok(Self {
            store: MemoryStore::new(PeerId::random()),
            metadata: HashMap::new(),
            config,
            expiry_timeout: NetworkTimeout::new(Duration::from_secs(3600))?, // 1 hour default
        })
    }

    /// Stores a record
    pub async fn put_record(&mut self, record: Record) -> RecordResult<()> {
        // Validate record size
        if record.value.len() > MAX_RECORD_SIZE {
            return Err(RecordError::SizeExceeded {
                size: record.value.len(),
                max_size: MAX_RECORD_SIZE,
            });
        }

        // Check if record has expired
        if let Some(expires) = record.expires {
            if expires <= SystemTime::now() {
                return Err(RecordError::Expired(expires));
            }
        }

        // Store metadata
        let metadata = RecordMetadata {
            stored_at: SystemTime::now(),
            publisher: record.publisher,
            size: record.value.len(),
            access_count: 0,
        };
        self.metadata.insert(record.key.clone(), metadata);

        // Store record
        self.store.put(record.key, record.value)
            .map_err(|e| RecordError::Storage(e.to_string()))?;

        debug!("Record stored successfully");
        Ok(())
    }

    /// Retrieves a record by key
    pub async fn get_record(&mut self, key: &[u8]) -> RecordResult<Record> {
        // Get record from store
        let value = self.store.get(key)
            .map_err(|e| RecordError::Storage(e.to_string()))?
            .ok_or(RecordError::NotFound)?;

        // Update metadata
        if let Some(metadata) = self.metadata.get_mut(key) {
            metadata.access_count += 1;
        }

        // Construct record
        let record = Record {
            key: key.to_vec(),
            value,
            publisher: self.metadata.get(key).and_then(|m| m.publisher),
            expires: None,
        };

        debug!("Record retrieved successfully");
        Ok(record)
    }

    /// Removes expired records
    pub async fn remove_expired(&mut self) -> Result<usize, NetworkError> {
        let now = SystemTime::now();
        let mut removed = 0;

        self.metadata.retain(|key, metadata| {
            let expired = now.duration_since(metadata.stored_at)
                .map(|age| age > self.expiry_timeout.duration())
                .unwrap_or(true);

            if expired {
                if let Err(e) = self.store.remove(key) {
                    warn!("Failed to remove expired record: {}", e);
                }
                removed += 1;
                false
            } else {
                true
            }
        });

        debug!("Removed {} expired records", removed);
        Ok(removed)
    }

    /// Returns the number of stored records
    pub fn record_count(&self) -> usize {
        self.metadata.len()
    }

    /// Returns the total size of stored records in bytes
    pub fn total_size(&self) -> usize {
        self.metadata.values().map(|m| m.size).sum()
    }

    /// Returns statistics about stored records
    pub fn stats(&self) -> RecordStats {
        RecordStats {
            record_count: self.record_count(),
            total_size: self.total_size(),
            total_accesses: self.metadata.values().map(|m| m.access_count).sum(),
        }
    }
}

/// Statistics about stored records
#[derive(Debug, Clone, Copy)]
pub struct RecordStats {
    /// Number of stored records
    pub record_count: usize,
    /// Total size of stored records in bytes
    pub total_size: usize,
    /// Total number of record accesses
    pub total_accesses: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_record_storage() {
        let config = NetworkConfig::default();
        let mut storage = RecordStorage::new(config).unwrap();

        // Store a record
        let record = Record {
            key: vec![1, 2, 3],
            value: vec![4, 5, 6],
            publisher: Some(PeerId::random()),
            expires: None,
        };

        storage.put_record(record.clone()).await.unwrap();
        assert_eq!(storage.record_count(), 1);

        // Retrieve the record
        let retrieved = storage.get_record(&record.key).await.unwrap();
        assert_eq!(retrieved.value, record.value);

        // Check stats
        let stats = storage.stats();
        assert_eq!(stats.record_count, 1);
        assert_eq!(stats.total_size, 3); // value length
        assert_eq!(stats.total_accesses, 1);
    }

    #[tokio::test]
    async fn test_record_size_limit() {
        let config = NetworkConfig::default();
        let mut storage = RecordStorage::new(config).unwrap();

        // Try to store a record that's too large
        let large_record = Record {
            key: vec![1],
            value: vec![0; MAX_RECORD_SIZE + 1],
            publisher: None,
            expires: None,
        };

        let result = storage.put_record(large_record).await;
        assert!(matches!(result, Err(RecordError::SizeExceeded { .. })));
    }

    #[tokio::test]
    async fn test_record_expiry() {
        let config = NetworkConfig::default();
        let mut storage = RecordStorage::new(config).unwrap();

        // Store a record that's already expired
        let expired_record = Record {
            key: vec![1],
            value: vec![2],
            publisher: None,
            expires: Some(SystemTime::now() - Duration::from_secs(1)),
        };

        let result = storage.put_record(expired_record).await;
        assert!(matches!(result, Err(RecordError::Expired(_))));

        // Store a valid record and wait for it to expire
        let record = Record {
            key: vec![1],
            value: vec![2],
            publisher: None,
            expires: None,
        };

        storage.put_record(record).await.unwrap();
        
        // Force expiry by manipulating metadata
        if let Some(metadata) = storage.metadata.get_mut(&vec![1]) {
            metadata.stored_at = SystemTime::now() - Duration::from_secs(3601);
        }

        let removed = storage.remove_expired().await.unwrap();
        assert_eq!(removed, 1);
        assert_eq!(storage.record_count(), 0);
    }

    #[tokio::test]
    async fn test_concurrent_record_access() {
        let config = NetworkConfig::default();
        let storage = Arc::new(RwLock::new(RecordStorage::new(config).unwrap()));
        let mut handles = vec![];

        // Create multiple records
        for i in 0..5 {
            let storage_clone = storage.clone();
            let record = Record {
                key: vec![i as u8],
                value: vec![i as u8 + 1],
                publisher: Some(PeerId::random()),
                expires: None,
            };

            handles.push(tokio::spawn(async move {
                storage_clone.write().await.put_record(record).await
            }));
        }

        // Wait for all puts to complete
        for handle in handles {
            handle.await.unwrap().unwrap();
        }

        let storage_guard = storage.read().await;
        assert_eq!(storage_guard.record_count(), 5);
    }

    #[tokio::test]
    async fn test_record_metadata() {
        let config = NetworkConfig::default();
        let mut storage = RecordStorage::new(config).unwrap();
        let publisher = PeerId::random();
        let key = vec![1];
        let value = vec![2, 3, 4];

        let record = Record {
            key: key.clone(),
            value: value.clone(),
            publisher: Some(publisher),
            expires: None,
        };

        // Store and retrieve multiple times
        storage.put_record(record.clone()).await.unwrap();
        for _ in 0..3 {
            storage.get_record(&key).await.unwrap();
        }

        let stats = storage.stats();
        assert_eq!(stats.record_count, 1);
        assert_eq!(stats.total_size, value.len());
        assert_eq!(stats.total_accesses, 3);
    }

    #[tokio::test]
    async fn test_record_expiry_batch() {
        let config = NetworkConfig::default();
        let mut storage = RecordStorage::new(config).unwrap();
        let now = SystemTime::now();
        let past = now - Duration::from_secs(7200); // 2 hours ago

        // Store multiple records with different expiry times
        for i in 0..5 {
            let record = Record {
                key: vec![i],
                value: vec![i + 1],
                publisher: None,
                expires: Some(if i % 2 == 0 { past } else { now + Duration::from_secs(3600) }),
            };

            if i % 2 == 1 {
                storage.put_record(record).await.unwrap();
            } else {
                // These should fail due to being expired
                assert!(matches!(
                    storage.put_record(record).await,
                    Err(RecordError::Expired(_))
                ));
            }
        }

        assert_eq!(storage.record_count(), 2);
    }

    #[tokio::test]
    async fn test_record_size_limits_edge_cases() {
        let config = NetworkConfig::default();
        let mut storage = RecordStorage::new(config).unwrap();

        // Test empty record
        let empty_record = Record {
            key: vec![1],
            value: vec![],
            publisher: None,
            expires: None,
        };
        storage.put_record(empty_record).await.unwrap();

        // Test maximum size record
        let max_record = Record {
            key: vec![2],
            value: vec![0; MAX_RECORD_SIZE],
            publisher: None,
            expires: None,
        };
        storage.put_record(max_record).await.unwrap();

        // Test one byte over maximum
        let too_large = Record {
            key: vec![3],
            value: vec![0; MAX_RECORD_SIZE + 1],
            publisher: None,
            expires: None,
        };
        assert!(matches!(
            storage.put_record(too_large).await,
            Err(RecordError::SizeExceeded { .. })
        ));
    }

    #[tokio::test]
    async fn test_record_cleanup() {
        let config = NetworkConfig::default();
        let mut storage = RecordStorage::new(config).unwrap();

        // Add some records
        for i in 0..5 {
            let record = Record {
                key: vec![i],
                value: vec![i + 1],
                publisher: None,
                expires: None,
            };
            storage.put_record(record).await.unwrap();
        }

        // Force expiry on some records
        for key in [0u8, 2u8, 4u8] {
            if let Some(metadata) = storage.metadata.get_mut(&vec![key]) {
                metadata.stored_at = SystemTime::now() - Duration::from_secs(3601);
            }
        }

        let removed = storage.remove_expired().await.unwrap();
        assert_eq!(removed, 3);
        assert_eq!(storage.record_count(), 2);

        // Verify remaining records
        for i in [1u8, 3u8] {
            assert!(storage.get_record(&vec![i]).await.is_ok());
        }
    }

    #[tokio::test]
    async fn test_record_overwrite() {
        let config = NetworkConfig::default();
        let mut storage = RecordStorage::new(config).unwrap();
        let key = vec![1];

        // Store initial record
        let record1 = Record {
            key: key.clone(),
            value: vec![2],
            publisher: None,
            expires: None,
        };
        storage.put_record(record1).await.unwrap();

        // Store new record with same key
        let record2 = Record {
            key: key.clone(),
            value: vec![3],
            publisher: None,
            expires: None,
        };
        storage.put_record(record2).await.unwrap();

        // Verify only the new value exists
        let retrieved = storage.get_record(&key).await.unwrap();
        assert_eq!(retrieved.value, vec![3]);
        assert_eq!(storage.record_count(), 1);
    }
} 