// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.
use crate::event::NetworkEvent;
use libp2p::{
    identity::PeerId,
    kad::{
        record::{Key, ProviderRecord, Record},
        store::{Error, RecordStore, Result},
        KBucketDistance as Distance, KBucketKey,
    },
};
use rand::Rng;
use sn_dbc::Token;
use sn_protocol::{NetworkAddress, PrettyPrintRecordKey};
use sn_transfers::dbc_genesis::TOTAL_SUPPLY;
use std::{
    borrow::Cow,
    collections::{hash_set, HashSet},
    fs,
    path::{Path, PathBuf},
    time::Duration,
    vec,
};
use tokio::sync::mpsc;
use xor_name::XorName;

// Each node will have a replication interval between these bounds
// This should serve to stagger the intense replication activity across the network
pub const REPLICATION_INTERVAL_UPPER_BOUND: Duration = Duration::from_secs(540);
pub const REPLICATION_INTERVAL_LOWER_BOUND: Duration = Duration::from_secs(180);

/// Max number of records a node can store
const MAX_RECORDS_COUNT: usize = 2048;

/// ~Number of puts per price step
const PUTS_PER_PRICE_STEP: usize = 100;

/// A `RecordStore` that stores records on disk.
pub struct DiskBackedRecordStore {
    /// The identity of the peer owning the store.
    local_key: KBucketKey<PeerId>,
    /// The configuration of the store.
    config: DiskBackedRecordStoreConfig,
    /// A set of keys, each corresponding to a data `Record` stored on disk.
    records: HashSet<Key>,
    /// Currently only used to notify the record received via network put to be validated.
    event_sender: Option<mpsc::Sender<NetworkEvent>>,
    /// Distance range specify the acceptable range of record entry.
    /// None means accept all records.
    distance_range: Option<Distance>,
}

/// Configuration for a `DiskBackedRecordStore`.
#[derive(Debug, Clone)]
pub struct DiskBackedRecordStoreConfig {
    /// The directory where the records are stored.
    pub storage_dir: PathBuf,
    /// The maximum number of records.
    pub max_records: usize,
    /// The maximum size of record values, in bytes.
    pub max_value_bytes: usize,
    /// This node's replication interval
    /// Which should be between REPLICATION_INTERVAL_LOWER_BOUND and REPLICATION_INTERVAL_UPPER_BOUND
    pub replication_interval: Duration,
}

impl Default for DiskBackedRecordStoreConfig {
    fn default() -> Self {
        // get a random integer between REPLICATION_INTERVAL_LOWER_BOUND and REPLICATION_INTERVAL_UPPER_BOUND
        let replication_interval = rand::thread_rng()
            .gen_range(REPLICATION_INTERVAL_LOWER_BOUND..REPLICATION_INTERVAL_UPPER_BOUND);

        Self {
            storage_dir: std::env::temp_dir(),
            max_records: MAX_RECORDS_COUNT,
            max_value_bytes: 65 * 1024,
            replication_interval,
        }
    }
}

impl DiskBackedRecordStore {
    /// Creates a new `DiskBackedStore` with the given configuration.
    pub fn with_config(
        local_id: PeerId,
        config: DiskBackedRecordStoreConfig,
        event_sender: Option<mpsc::Sender<NetworkEvent>>,
    ) -> Self {
        DiskBackedRecordStore {
            local_key: KBucketKey::from(local_id),
            config,
            records: Default::default(),
            event_sender,
            distance_range: None,
        }
    }

    /// Returns `true` if the `Key` is present locally
    pub fn contains(&self, key: &Key) -> bool {
        self.records.contains(key)
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

    pub fn record_addresses(&self) -> HashSet<NetworkAddress> {
        self.records
            .iter()
            .map(|record_key| NetworkAddress::from_record_key(record_key.clone()))
            .collect()
    }

    #[allow(clippy::mutable_key_type)]
    pub fn record_addresses_ref(&self) -> &HashSet<Key> {
        &self.records
    }

    pub fn read_from_disk<'a>(key: &Key, storage_dir: &Path) -> Option<Cow<'a, Record>> {
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

    pub fn put_verified(&mut self, r: Record) -> Result<()> {
        let content_hash = XorName::from_content(&r.value);
        let record_key = PrettyPrintRecordKey::from(r.key.clone());
        trace!("PUT a verified Record: {record_key:?} (content_hash {content_hash:?})");

        self.prune_storage_if_needed_for_record(&r.key)?;

        let filename = Self::key_to_hex(&r.key);
        let file_path = self.config.storage_dir.join(&filename);
        let _ = self.records.insert(r.key);

        // TODO: How could we clean up records if we fail to insert?
        tokio::spawn(async move {
            match fs::write(file_path, r.value) {
                Ok(_) => {
                    trace!("Wrote record {record_key:?} to disk! filename: {filename}");
                }
                Err(err) => {
                    error!(
                        "Error writing record {record_key:?} filename: {filename}, error: {err:?}"
                    );
                }
            }
        });

        Ok(())
    }

    /// Prune the records in the store to ensure that we free up space
    /// for the incoming record.
    ///
    /// An error is returned if we are full and the new record is not closer than
    /// the furthest record
    fn prune_storage_if_needed_for_record(&mut self, r: &Key) -> Result<()> {
        let num_records = self.records.len();

        // we're not full, so we don't need to prune
        if num_records < self.config.max_records {
            return Ok(());
        }

        // sort records by distance to our local key
        let mut records = self.records.iter().cloned().collect::<Vec<_>>();
        records.sort_unstable_by_key(|k| {
            let kbucket_key = KBucketKey::from(k.to_vec());
            self.local_key.distance(&kbucket_key)
        });

        // now check if the incoming record is closer than our furthest
        // if it is, we can prune
        if let Some(furthest_record) = records.last() {
            let furthest_record_key = KBucketKey::from(furthest_record.to_vec());
            let incoming_record_key = KBucketKey::from(r.to_vec());

            if incoming_record_key.distance(&self.local_key)
                < furthest_record_key.distance(&self.local_key)
            {
                trace!(
                    "{:?} will be pruned to make space for new record: {:?}",
                    PrettyPrintRecordKey::from(furthest_record.clone()),
                    PrettyPrintRecordKey::from(r.clone())
                );
                // we should prune and make space
                self.remove(furthest_record);

                // Warn if the furthest record was within our distance range
                if let Some(distance_range) = self.distance_range {
                    if furthest_record_key.distance(&self.local_key) < distance_range {
                        warn!("Pruned record would also be within our distance range.");
                    }
                }
            } else {
                // we should not prune, but warn as we're at max capcaity
                warn!("Record not stored. Maximum number of records reached. Current num_records: {num_records}");
                return Err(Error::MaxRecords);
            }
        }

        Ok(())
    }

    #[allow(dead_code)]
    /// Calculate the cost to store data for our current store state
    pub fn store_cost(&self) -> Token {
        // Calculate the factor to increase the cost for every PUTS_PER_PRICE_STEP records
        let factor =
            10.0f64.powf(MAX_RECORDS_COUNT as f64 / PUTS_PER_PRICE_STEP as f64 - 2.0_f64) as u64;

        // Calculate the starting cost
        let mut cost = TOTAL_SUPPLY / factor;

        trace!("Starting cost is {:?}", cost);

        trace!("Record count is {:?}", self.records.len());
        let relevant_records_len = if let Some(distance_range) = self.distance_range {
            self.records
                .iter()
                .filter(|key| {
                    let kbucket_key = KBucketKey::from(key.to_vec());
                    distance_range >= self.local_key.distance(&kbucket_key)
                })
                .count()
        } else {
            // Otherwise we've no distance range set, so we actually don't know enough
            // so we'll say we have MAX_RECORDS_COUNT and set a high price until we know
            // more about our CLOSE_GROUP
            MAX_RECORDS_COUNT
        };

        trace!("Relevant records len is {:?}", relevant_records_len);

        // Find where we are on the scale
        let current_step = relevant_records_len / PUTS_PER_PRICE_STEP + 1;

        trace!("Current step is {:?}", current_step);

        // Double the cost for each step up to the current step
        for _i in 0..current_step {
            cost = cost.saturating_add(cost);
        }

        trace!("Cost is now {:?}", cost);
        Token::from_nano(cost)
    }

    /// Setup the distance range.
    pub fn set_distance_range(&mut self, distance_range: Distance) {
        self.distance_range = Some(distance_range);
    }
}

impl RecordStore for DiskBackedRecordStore {
    type RecordsIter<'a> = RecordsIterator<'a>;
    type ProvidedIter<'a> = vec::IntoIter<Cow<'a, ProviderRecord>>;

    fn get(&self, k: &Key) -> Option<Cow<'_, Record>> {
        // When a client calls GET, the request is forwarded to the nodes until one node returns
        // with the record. Thus a node can be bombarded with GET reqs for random keys. These can be safely
        // ignored if we don't have the record locally.
        trace!(
            "GET request for Record key: {:?}",
            PrettyPrintRecordKey::from(k.clone())
        );
        if !self.records.contains(k) {
            trace!("Record not found locally");
            return None;
        }

        Self::read_from_disk(k, &self.config.storage_dir)
    }

    fn put(&mut self, record: Record) -> Result<()> {
        if record.value.len() >= self.config.max_value_bytes {
            warn!(
                "Record not stored. Value too large: {} bytes",
                record.value.len()
            );
            return Err(Error::ValueTooLarge);
        }

        if self.records.contains(&record.key) {
            trace!(
                "Unverified Record {:?} already exists.",
                PrettyPrintRecordKey::from(record.key.clone())
            );

            // Blindly sent to validation to allow double spend can be detected.
            // TODO: consider avoid throw duplicated chunk to validation.
        }
        trace!(
            "Unverified Record {:?} try to validate and store",
            PrettyPrintRecordKey::from(record.key.clone())
        );
        if let Some(event_sender) = self.event_sender.clone() {
            // push the event off thread so as to be non-blocking
            let _handle = tokio::spawn(async move {
                if let Err(error) = event_sender
                    .send(NetworkEvent::UnverifiedRecord(record))
                    .await
                {
                    error!("SwarmDriver failed to send event: {}", error);
                }
            });
        } else {
            error!("Record store doesn't have event_sender setup");
        }
        Ok(())
    }

    fn remove(&mut self, k: &Key) {
        let _ = self.records.remove(k);

        let filename = Self::key_to_hex(k);
        let file_path = self.config.storage_dir.join(&filename);

        let _handle = tokio::spawn(async move {
            match fs::remove_file(file_path) {
                Ok(_) => {
                    trace!("Removed record from disk! filename: {filename}");
                }
                Err(err) => {
                    error!("Error while removing file. filename: {filename}, error: {err:?}");
                }
            }
        });
    }

    // A backstop replication shall only trigger within pre-defined interval
    fn records(&self) -> Self::RecordsIter<'_> {
        RecordsIterator {
            keys: self.records.iter(),
            storage_dir: self.config.storage_dir.clone(),
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

// Since 'Record's need to be read from disk for each individual 'Key', we need this iterator
// which does that operation at the very moment the consumer/user is iterating each item.
pub struct RecordsIterator<'a> {
    keys: hash_set::Iter<'a, Key>,
    storage_dir: PathBuf,
}

impl<'a> Iterator for RecordsIterator<'a> {
    type Item = Cow<'a, Record>;

    fn next(&mut self) -> Option<Self::Item> {
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
    use libp2p::{
        core::multihash::Multihash,
        kad::{KBucketKey, RecordKey},
    };
    use quickcheck::*;
    use tokio::runtime::Runtime;

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
                Multihash::<64>::wrap(MULITHASH_CODE, &hash).expect("Failed to gen MultiHash"),
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
            let rt = if let Ok(rt) = Runtime::new() {
                rt
            } else {
                panic!("Cannot create runtime");
            };
            rt.block_on(testing_thread(r));
        }
        quickcheck(prop as fn(_))
    }

    async fn testing_thread(r: ArbitraryRecord) {
        let r = r.0;
        let (network_event_sender, mut network_event_receiver) = mpsc::channel(1);
        let mut store = DiskBackedRecordStore::with_config(
            PeerId::random(),
            Default::default(),
            Some(network_event_sender),
        );

        let store_cost_before = store.store_cost();
        // An initial unverified put should not write to disk
        assert!(store.put(r.clone()).is_ok());
        assert!(store.get(&r.key).is_none());
        // Store cost should not change if no PUT has been added
        assert_eq!(
            store.store_cost(),
            store_cost_before,
            "store cost should not change over unverified put"
        );

        let returned_record = if let Some(event) = network_event_receiver.recv().await {
            if let NetworkEvent::UnverifiedRecord(record) = event {
                record
            } else {
                panic!("Unexpected network event {event:?}");
            }
        } else {
            panic!("Failed recevied the record for further verification");
        };

        assert!(store.put_verified(returned_record).is_ok());

        // loop over store.get max_iterations times to ensure async disk write had time to complete.
        let max_iterations = 10;
        let mut iteration = 0;
        while iteration < max_iterations {
            // try to check if it is equal to the actual record. This is needed because, the file
            // might not be fully written to the fs and would cause intermittent failures.
            // If there is actually a problem with the PUT, the assert statement below would catch it.
            if store
                .get(&r.key)
                .is_some_and(|record| Cow::Borrowed(&r) == record)
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
            iteration += 1;
        }
        if iteration == max_iterations {
            panic!("record_store test failed with stored record cann't be read back");
        }

        assert_eq!(
            Some(Cow::Borrowed(&r)),
            store.get(&r.key),
            "record can be retrieved after put"
        );
        store.remove(&r.key);

        assert!(store.get(&r.key).is_none());
    }

    #[tokio::test]
    async fn pruning_on_full() -> Result<()> {
        let max_iterations = 10;
        let max_records = 50;

        // Set the config::max_record to be 50, then generate 100 records
        // On storing the 51st to 100th record,
        // check there is an expected pruning behaviour got carried out.
        let store_config = DiskBackedRecordStoreConfig {
            max_records,
            ..Default::default()
        };
        let self_id = PeerId::random();
        let mut store = DiskBackedRecordStore::with_config(self_id, store_config.clone(), None);

        let mut stored_records: Vec<RecordKey> = vec![];
        let self_address = NetworkAddress::from_peer(self_id);
        for i in 0..100 {
            let record_key = NetworkAddress::from_peer(PeerId::random()).to_record_key();
            let record = Record {
                key: record_key.clone(),
                value: (0..50).map(|_| rand::random::<u8>()).collect(),
                publisher: None,
                expires: None,
            };
            let retained_key = if i < max_records {
                assert!(store.put_verified(record).is_ok());
                record_key
            } else {
                // The list is already sorted by distance, hence always shall only prune the last one
                let furthest_key = stored_records.remove(stored_records.len() - 1);
                let furthest_addr = NetworkAddress::from_record_key(furthest_key.clone());
                let record_addr = NetworkAddress::from_record_key(record_key.clone());
                let (retained_key, pruned_key) = if self_address.distance(&furthest_addr)
                    > self_address.distance(&record_addr)
                {
                    // The new entry is closer, it shall replace the existing one
                    assert!(store.put_verified(record).is_ok());
                    (record_key, furthest_key)
                } else {
                    // The new entry is farther away, it shall not replace the existing one
                    assert!(store.put_verified(record).is_err());
                    (furthest_key, record_key)
                };

                // Confirm the pruned_key got removed, looping to allow async disk ops to complete.
                let mut iteration = 0;
                while iteration < max_iterations {
                    if DiskBackedRecordStore::read_from_disk(&pruned_key, &store_config.storage_dir)
                        .is_none()
                    {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    iteration += 1;
                }
                if iteration == max_iterations {
                    panic!("record_store prune test failed with pruned record still exists.");
                }

                retained_key
            };

            // loop over max_iterations times to ensure async disk write had time to complete.
            let mut iteration = 0;
            while iteration < max_iterations {
                if store.get(&retained_key).is_some() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
                iteration += 1;
            }
            if iteration == max_iterations {
                panic!("record_store prune test failed with stored record cann't be read back");
            }

            stored_records.push(retained_key);
            stored_records.sort_by(|a, b| {
                let a = NetworkAddress::from_record_key(a.clone());
                let b = NetworkAddress::from_record_key(b.clone());
                self_address.distance(&a).cmp(&self_address.distance(&b))
            });
        }

        Ok(())
    }
}
