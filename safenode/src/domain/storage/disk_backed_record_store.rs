// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use libp2p::{
    identity::PeerId,
    kad::{
        kbucket::{Distance, Key as KBucketKey},
        record::{Key, ProviderRecord, Record},
        store::{Error, RecordStore, Result},
    },
};
use rand::Rng;
use std::{
    borrow::Cow,
    collections::{hash_set, HashSet},
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
    vec,
};

// Each node will have a replication interval between these bounds
// This should serve to stagger the intense replication activity across the network
pub(crate) const REPLICATION_INTERVAL_UPPER_BOUND: Duration = Duration::from_secs(540);
pub(crate) const REPLICATION_INTERVAL_LOWER_BOUND: Duration = Duration::from_secs(180);

/// A `RecordStore` that stores records on disk.
pub(crate) struct DiskBackedRecordStore {
    /// The identity of the peer owning the store.
    #[allow(dead_code)]
    local_key: KBucketKey<PeerId>,
    /// The configuration of the store.
    config: DiskBackedRecordStoreConfig,
    /// A set of keys, each corresponding to a data `Record` stored on disk.
    records: HashSet<Key>,
    /// Time that replication triggered.
    replication_start: Instant,
}

/// Configuration for a `DiskBackedRecordStore`.
#[derive(Debug, Clone)]
pub(crate) struct DiskBackedRecordStoreConfig {
    /// The directory where the records are stored.
    pub(crate) storage_dir: PathBuf,
    /// The maximum number of records.
    pub(crate) max_records: usize,
    /// The maximum size of record values, in bytes.
    pub(crate) max_value_bytes: usize,
    /// This node's replication interval
    /// Which should be between REPLICATION_INTERVAL_LOWER_BOUND and REPLICATION_INTERVAL_UPPER_BOUND
    pub(crate) replication_interval: Duration,
}

impl Default for DiskBackedRecordStoreConfig {
    fn default() -> Self {
        // get a random integer between REPLICATION_INTERVAL_LOWER_BOUND and REPLICATION_INTERVAL_UPPER_BOUND
        let replication_interval = rand::thread_rng()
            .gen_range(REPLICATION_INTERVAL_LOWER_BOUND..REPLICATION_INTERVAL_UPPER_BOUND);

        Self {
            storage_dir: std::env::temp_dir(),
            max_records: 1024,
            max_value_bytes: 65 * 1024,
            replication_interval,
        }
    }
}

impl DiskBackedRecordStore {
    /// Creates a new `DiskBackedStore` with a default configuration.
    #[allow(dead_code)]
    pub(crate) fn new(local_id: PeerId) -> Self {
        Self::with_config(local_id, Default::default())
    }

    /// Creates a new `DiskBackedStore` with the given configuration.
    pub(crate) fn with_config(local_id: PeerId, config: DiskBackedRecordStoreConfig) -> Self {
        DiskBackedRecordStore {
            local_key: KBucketKey::from(local_id),
            config,
            records: Default::default(),
            replication_start: Instant::now(),
        }
    }

    /// Retains the records satisfying a predicate.
    #[allow(dead_code)]
    pub(crate) fn retain<F>(&mut self, predicate: F)
    where
        F: Fn(&Key) -> bool,
    {
        let to_be_removed = self
            .records
            .iter()
            .filter(|k| !predicate(k))
            .cloned()
            .collect::<Vec<_>>();

        to_be_removed.iter().for_each(|key| self.remove(key));
    }

    /// Returns the list of keys that within the distance to the target
    pub(crate) fn entries_to_be_replicated(
        &mut self,
        target: KBucketKey<Vec<u8>>,
        distance_bar: Distance,
    ) -> Vec<Key> {
        self.replication_start = Instant::now();
        self.records
            .iter()
            .filter(|key| {
                let record_key = KBucketKey::from(key.to_vec());
                target.distance(&record_key) < distance_bar
            })
            .cloned()
            .collect()
    }

    // Converts a Key into a Hex string.
    fn key_to_hex(key: &Key) -> String {
        let key_bytes = key.as_ref();
        let mut hex_string = String::with_capacity(key_bytes.len() * 2);
        for byte in key_bytes {
            hex_string.push_str(&format!("{:02x}", byte));
        }
        hex_string
    }

    pub(crate) fn storage_dir(&self) -> PathBuf {
        self.config.storage_dir.clone()
    }

    pub(crate) fn read_from_disk<'a>(key: &Key, storage_dir: &Path) -> Option<Cow<'a, Record>> {
        let filename = Self::key_to_hex(key);
        let file_path = storage_dir.join(&filename);

        match fs::read(file_path) {
            Ok(value) => {
                trace!("Retrieved record from disk! filename: {filename}");
                let record = Record {
                    key: key.clone(),
                    value,
                    publisher: None,
                    expires: None,
                };
                Some(Cow::Owned(record))
            }
            Err(err) => {
                error!("Error while reading file. filename: {filename}, error: {err:?}");
                None
            }
        }
    }

    pub(crate) fn write_to_local(&mut self, record: Record) -> Result<()> {
        self.put(record)
    }
}

impl RecordStore for DiskBackedRecordStore {
    type RecordsIter<'a> = RecordsIterator<'a>;
    type ProvidedIter<'a> = vec::IntoIter<Cow<'a, ProviderRecord>>;

    fn get(&self, k: &Key) -> Option<Cow<'_, Record>> {
        // When a client calls GET, the request is forwarded to the nodes until one node returns
        // with the record. Thus a node can be bombarded with GET reqs for random keys. These can be safely
        // ignored if we don't have the record locally.
        trace!("GET request for Record key: {k:?}");
        if !self.records.contains(k) {
            trace!("Record not found locally");
            return None;
        }

        Self::read_from_disk(k, &self.config.storage_dir)
    }

    fn put(&mut self, r: Record) -> Result<()> {
        trace!("PUT request for Record key: {:?}", r.key);
        if r.value.len() >= self.config.max_value_bytes {
            warn!(
                "Record not stored. Value too large: {} bytes",
                r.value.len()
            );
            return Err(Error::ValueTooLarge);
        }

        // todo: the key is not tied to the value it contains, hence the value can be overwritten
        // (incase of dbc double spends etc), hence need to deal with those.
        // Maybe implement a RecordHeader to store the type of data we're storing?
        if self.records.contains(&r.key) {
            debug!(
                "Record with key {:?} already exists, not overwriting.",
                r.key
            );
            return Ok(());
        }

        let num_records = self.records.len();
        if num_records >= self.config.max_records {
            warn!("Record not stored. Maximum number of records reached. Current num_records: {num_records}");
            return Err(Error::MaxRecords);
        }

        let filename = Self::key_to_hex(&r.key);
        let file_path = self.config.storage_dir.join(&filename);
        match fs::write(file_path, r.value) {
            Ok(_) => {
                trace!("Wrote record to disk! filename: {filename}");
                let _ = self.records.insert(r.key);
                Ok(())
            }
            Err(err) => {
                error!("Error writing file. filename: {filename}, error: {err:?}");
                Ok(())
            }
        }
    }

    fn remove(&mut self, k: &Key) {
        let _ = self.records.remove(k);

        let filename = Self::key_to_hex(k);
        let file_path = self.config.storage_dir.join(&filename);
        match fs::remove_file(file_path) {
            Ok(_) => {
                trace!("Removed record from disk! filename: {filename}");
            }
            Err(err) => {
                error!("Error while removing file. filename: {filename}, error: {err:?}");
            }
        }
    }

    // A backstop replication shall only trigger within pre-defined interval
    fn records(&self) -> Self::RecordsIter<'_> {
        trace!(
            "Periodic replication shall be triggered: {:?}",
            self.replication_start + self.config.replication_interval < Instant::now()
        );
        RecordsIterator {
            keys: self.records.iter(),
            storage_dir: self.config.storage_dir.clone(),
            // Set to false to ensure periodic replication disabled.
            is_triggered: false,
        }
    }

    fn add_provider(&mut self, _record: ProviderRecord) -> Result<()> {
        // ProviderRecords are not used currently
        Ok(())
    }

    fn providers(&self, _key: &Key) -> Vec<ProviderRecord> {
        // ProviderRecords are not used currently
        vec![]
    }

    fn provided(&self) -> Self::ProvidedIter<'_> {
        // ProviderRecords are not used currently
        vec![].into_iter()
    }

    fn remove_provider(&mut self, _key: &Key, _provider: &PeerId) {
        // ProviderRecords are not used currently
    }
}

// Since 'Record's need to be read from disk for each indiviaul 'Key', we need this iterator
// which does that operation at the very moment the consumer/user is iterating each item.
pub(crate) struct RecordsIterator<'a> {
    keys: hash_set::Iter<'a, Key>,
    storage_dir: PathBuf,
    is_triggered: bool,
}

impl<'a> Iterator for RecordsIterator<'a> {
    type Item = Cow<'a, Record>;

    fn next(&mut self) -> Option<Self::Item> {
        if !self.is_triggered {
            return None;
        }
        for key in self.keys.by_ref() {
            let record = DiskBackedRecordStore::read_from_disk(key, &self.storage_dir);
            if record.is_some() {
                return record;
            }
        }

        None
    }
}

#[allow(trivial_casts)]
#[cfg(test)]
mod tests {
    use super::*;
    use libp2p::{core::multihash::Multihash, kad::kbucket::Key as KBucketKey};
    use quickcheck::*;

    const MULITHASH_CODE: u64 = 0x12;

    #[derive(Clone, Debug)]
    struct ArbitraryKey(Key);
    #[derive(Clone, Debug)]
    struct ArbitraryPeerId(PeerId);
    #[derive(Clone, Debug)]
    struct ArbitraryKBucketKey(KBucketKey<PeerId>);
    #[derive(Clone, Debug)]
    struct ArbitraryRecord(Record);
    #[derive(Clone, Debug)]
    struct ArbitraryProviderRecord(ProviderRecord);

    impl Arbitrary for ArbitraryPeerId {
        fn arbitrary(g: &mut Gen) -> ArbitraryPeerId {
            let hash: [u8; 32] = core::array::from_fn(|_| u8::arbitrary(g));
            let peer_id = PeerId::from_multihash(
                Multihash::wrap(MULITHASH_CODE, &hash).expect("Failed to gen Multihash"),
            )
            .expect("Failed to create PeerId");
            ArbitraryPeerId(peer_id)
        }
    }

    impl Arbitrary for ArbitraryKBucketKey {
        fn arbitrary(_: &mut Gen) -> ArbitraryKBucketKey {
            ArbitraryKBucketKey(KBucketKey::from(PeerId::random()))
        }
    }

    impl Arbitrary for ArbitraryKey {
        fn arbitrary(g: &mut Gen) -> ArbitraryKey {
            let hash: [u8; 32] = core::array::from_fn(|_| u8::arbitrary(g));
            ArbitraryKey(Key::from(
                Multihash::wrap(MULITHASH_CODE, &hash).expect("Failed to gen MultiHash"),
            ))
        }
    }

    impl Arbitrary for ArbitraryRecord {
        fn arbitrary(g: &mut Gen) -> ArbitraryRecord {
            let record = Record {
                key: ArbitraryKey::arbitrary(g).0,
                value: Vec::arbitrary(g),
                publisher: None,
                expires: None,
            };
            ArbitraryRecord(record)
        }
    }

    impl Arbitrary for ArbitraryProviderRecord {
        fn arbitrary(g: &mut Gen) -> ArbitraryProviderRecord {
            let record = ProviderRecord {
                key: ArbitraryKey::arbitrary(g).0,
                provider: PeerId::random(),
                expires: None,
                addresses: vec![],
            };
            ArbitraryProviderRecord(record)
        }
    }

    #[test]
    fn put_get_remove_record() {
        fn prop(r: ArbitraryRecord) {
            let r = r.0;
            let mut store = DiskBackedRecordStore::new(PeerId::random());
            assert!(store.put(r.clone()).is_ok());
            assert_eq!(Some(Cow::Borrowed(&r)), store.get(&r.key));
            store.remove(&r.key);
            assert!(store.get(&r.key).is_none());
        }
        quickcheck(prop as fn(_))
    }
}
