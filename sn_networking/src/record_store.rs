// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.
#![allow(clippy::mutable_key_type)] // for the Bytes in NetworkAddress

use crate::event::NetworkEvent;
use libp2p::{
    identity::PeerId,
    kad::{
        store::{Error, RecordStore, Result},
        KBucketDistance as Distance, KBucketKey, ProviderRecord, Record, RecordKey as Key,
    },
};
#[cfg(feature = "open-metrics")]
use prometheus_client::metrics::gauge::Gauge;
use sn_protocol::{
    storage::{RecordHeader, RecordKind, RecordType},
    NetworkAddress, PrettyPrintRecordKey,
};
use sn_transfers::NanoTokens;
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    vec,
};
use tokio::sync::mpsc;
use xor_name::XorName;

/// Max number of records a node can store
const MAX_RECORDS_COUNT: usize = 2048;

/// A `RecordStore` that stores records on disk.
pub struct NodeRecordStore {
    /// The identity of the peer owning the store.
    local_key: KBucketKey<PeerId>,
    /// The configuration of the store.
    config: NodeRecordStoreConfig,
    /// A set of keys, each corresponding to a data `Record` stored on disk.
    records: HashMap<Key, (NetworkAddress, RecordType)>,
    /// Currently only used to notify the record received via network put to be validated.
    event_sender: Option<mpsc::Sender<NetworkEvent>>,
    /// Distance range specify the acceptable range of record entry.
    /// None means accept all records.
    distance_range: Option<Distance>,
    #[cfg(feature = "open-metrics")]
    /// Used to report the number of records held by the store to the metrics server.
    record_count_metric: Option<Gauge>,
}

/// Configuration for a `DiskBackedRecordStore`.
#[derive(Debug, Clone)]
pub struct NodeRecordStoreConfig {
    /// The directory where the records are stored.
    pub storage_dir: PathBuf,
    /// The maximum number of records.
    pub max_records: usize,
    /// The maximum size of record values, in bytes.
    pub max_value_bytes: usize,
}

impl Default for NodeRecordStoreConfig {
    fn default() -> Self {
        Self {
            storage_dir: std::env::temp_dir(),
            max_records: MAX_RECORDS_COUNT,
            max_value_bytes: 65 * 1024,
        }
    }
}

impl NodeRecordStore {
    /// Creates a new `DiskBackedStore` with the given configuration.
    pub fn with_config(
        local_id: PeerId,
        config: NodeRecordStoreConfig,
        event_sender: Option<mpsc::Sender<NetworkEvent>>,
    ) -> Self {
        NodeRecordStore {
            local_key: KBucketKey::from(local_id),
            config,
            records: Default::default(),
            event_sender,
            distance_range: None,
            #[cfg(feature = "open-metrics")]
            record_count_metric: None,
        }
    }

    /// Set the record_count_metric to report the number of records stored to the metrics server
    #[cfg(feature = "open-metrics")]
    pub fn set_record_count_metric(mut self, metric: Gauge) -> Self {
        self.record_count_metric = Some(metric);
        self
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

    fn read_from_disk<'a>(key: &Key, storage_dir: &Path) -> Option<Cow<'a, Record>> {
        let filename = Self::key_to_hex(key);
        let file_path = storage_dir.join(&filename);

        match fs::read(file_path) {
            Ok(value) => {
                debug!("Retrieved record from disk! filename: {filename}");
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
        let furthest = self
            .records
            .keys()
            .max_by_key(|k| {
                let kbucket_key = KBucketKey::from(k.to_vec());
                self.local_key.distance(&kbucket_key)
            })
            .cloned();

        // now check if the incoming record is closer than our furthest
        // if it is, we can prune
        if let Some(furthest_record) = furthest {
            let furthest_record_key = KBucketKey::from(furthest_record.to_vec());
            let incoming_record_key = KBucketKey::from(r.to_vec());

            if incoming_record_key.distance(&self.local_key)
                < furthest_record_key.distance(&self.local_key)
            {
                trace!(
                    "{:?} will be pruned to make space for new record: {:?}",
                    PrettyPrintRecordKey::from(&furthest_record),
                    PrettyPrintRecordKey::from(r)
                );
                // we should prune and make space
                self.remove(&furthest_record);

                // Warn if the furthest record was within our distance range
                if let Some(distance_range) = self.distance_range {
                    if furthest_record_key.distance(&self.local_key) < distance_range {
                        warn!("Pruned record would also be within our distance range.");
                    }
                }
            } else {
                // we should not prune, but warn as we're at max capacity
                warn!("Record not stored. Maximum number of records reached. Current num_records: {num_records}");
                return Err(Error::MaxRecords);
            }
        }

        Ok(())
    }
}

impl NodeRecordStore {
    /// Returns `true` if the `Key` is present locally
    pub(crate) fn contains(&self, key: &Key) -> bool {
        self.records.contains_key(key)
    }

    /// Returns the set of `NetworkAddress::RecordKey` held by the store
    /// Use `record_addresses_ref` to get a borrowed type
    pub(crate) fn record_addresses(&self) -> HashMap<NetworkAddress, RecordType> {
        self.records
            .iter()
            .map(|(_record_key, (addr, record_type))| (addr.clone(), record_type.clone()))
            .collect()
    }

    /// Returns the reference to the set of `NetworkAddress::RecordKey` held by the store
    #[allow(clippy::mutable_key_type)]
    pub(crate) fn record_addresses_ref(&self) -> &HashMap<Key, (NetworkAddress, RecordType)> {
        &self.records
    }

    /// Warning: PUTs a `Record` to the store without validation
    /// Should be used in context where the `Record` is trusted
    pub(crate) fn put_verified(&mut self, r: Record, record_type: RecordType) -> Result<()> {
        let record_key = PrettyPrintRecordKey::from(&r.key).into_owned();
        trace!("PUT a verified Record: {record_key:?}");

        self.prune_storage_if_needed_for_record(&r.key)?;

        let filename = Self::key_to_hex(&r.key);
        let file_path = self.config.storage_dir.join(&filename);
        let _ = self.records.insert(
            r.key.clone(),
            (NetworkAddress::from_record_key(&r.key), record_type),
        );
        #[cfg(feature = "open-metrics")]
        if let Some(metric) = &self.record_count_metric {
            let _ = metric.set(self.records.len() as i64);
        }

        let cloned_event_sender = self.event_sender.clone();

        tokio::spawn(async move {
            match fs::write(&file_path, r.value) {
                Ok(_) => {
                    info!("Wrote record {record_key:?} to disk! filename: {filename}");
                }
                Err(err) => {
                    error!(
                        "Error writing record {record_key:?} filename: {filename}, error: {err:?}"
                    );

                    if let Some(event_sender) = cloned_event_sender {
                        if let Err(error) =
                            event_sender.send(NetworkEvent::FailedToWrite(r.key)).await
                        {
                            error!("SwarmDriver failed to send event: {}", error);
                        }
                    } else {
                        error!("Record store doesn't have event_sender could not log failed write to disk for {file_path:?}");
                    }
                }
            }
        });

        Ok(())
    }

    /// Calculate the cost to store data for our current store state
    #[allow(clippy::mutable_key_type)]
    pub(crate) fn store_cost(&self) -> NanoTokens {
        let relevant_records_len = if let Some(distance_range) = self.distance_range {
            let record_keys: HashSet<_> = self.records.keys().cloned().collect();
            self.get_records_within_distance_range(&record_keys, distance_range)
        } else {
            warn!("No distance range set on record store. Returning MAX_RECORDS_COUNT for relevant records in store cost calculation.");
            MAX_RECORDS_COUNT
        };

        let cost = calculate_cost_for_relevant_records(relevant_records_len);

        debug!("Cost is now {cost:?}");
        NanoTokens::from(cost)
    }

    /// Calculate how many records are stored within a distance range
    #[allow(clippy::mutable_key_type)]
    pub fn get_records_within_distance_range(
        &self,
        records: &HashSet<Key>,
        distance_range: Distance,
    ) -> usize {
        debug!(
            "Total record count is {:?}. Distance is: {distance_range:?}",
            self.records.len()
        );

        let relevant_records_len = records
            .iter()
            .filter(|key| {
                let kbucket_key = KBucketKey::new(key.to_vec());
                distance_range >= self.local_key.distance(&kbucket_key)
            })
            .count();

        debug!("Relevant records len is {:?}", relevant_records_len);
        relevant_records_len
    }

    /// Setup the distance range.
    pub(crate) fn set_distance_range(&mut self, distance_range: Distance) {
        self.distance_range = Some(distance_range);
    }
}

impl RecordStore for NodeRecordStore {
    type RecordsIter<'a> = vec::IntoIter<Cow<'a, Record>>;
    type ProvidedIter<'a> = vec::IntoIter<Cow<'a, ProviderRecord>>;

    fn get(&self, k: &Key) -> Option<Cow<'_, Record>> {
        // When a client calls GET, the request is forwarded to the nodes until one node returns
        // with the record. Thus a node can be bombarded with GET reqs for random keys. These can be safely
        // ignored if we don't have the record locally.
        let key = PrettyPrintRecordKey::from(k);
        if !self.records.contains_key(k) {
            trace!("Record not found locally: {key}");
            return None;
        }

        debug!("GET request for Record key: {key}");

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

        let record_key = PrettyPrintRecordKey::from(&record.key);

        // Record with payment shall always get passed further
        // to allow the payment to be taken and credit into own wallet.
        match RecordHeader::from_record(&record) {
            Ok(record_header) => {
                match record_header.kind {
                    RecordKind::ChunkWithPayment | RecordKind::RegisterWithPayment => {
                        trace!("Record {record_key:?} with payment shall always be processed.");
                    }
                    _ => {
                        // Chunk with existing key do not to be stored again.
                        // `Spend` or `Register` with same content_hash do not to be stored again,
                        // otherwise shall be passed further to allow
                        // double spend to be detected or register op update.
                        match self.records.get(&record.key) {
                            Some((_addr, RecordType::Chunk)) => {
                                trace!("Chunk {record_key:?} already exists.");
                                return Ok(());
                            }
                            Some((_addr, RecordType::NonChunk(existing_content_hash))) => {
                                let content_hash = XorName::from_content(&record.value);
                                if content_hash == *existing_content_hash {
                                    trace!("A non-chunk record {record_key:?} with same content_hash {content_hash:?} already exists.");
                                    return Ok(());
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            Err(err) => {
                error!("For record {record_key:?}, failed to parse record_header {err:?}");
                return Ok(());
            }
        }

        trace!("Unverified Record {record_key:?} try to validate and store");
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
        #[cfg(feature = "open-metrics")]
        if let Some(metric) = &self.record_count_metric {
            let _ = metric.set(self.records.len() as i64);
        }

        let filename = Self::key_to_hex(k);
        let file_path = self.config.storage_dir.join(&filename);

        let _handle = tokio::spawn(async move {
            match fs::remove_file(file_path) {
                Ok(_) => {
                    info!("Removed record from disk! filename: {filename}");
                }
                Err(err) => {
                    error!("Error while removing file. filename: {filename}, error: {err:?}");
                }
            }
        });
    }

    fn records(&self) -> Self::RecordsIter<'_> {
        // the records iter is used only during kad replication which is turned off
        vec![].into_iter()
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

/// A place holder RecordStore impl for the client that does nothing
#[derive(Default, Debug)]
pub struct ClientRecordStore {
    empty_record_addresses: HashMap<Key, (NetworkAddress, RecordType)>,
}

impl ClientRecordStore {
    pub(crate) fn contains(&self, _key: &Key) -> bool {
        false
    }

    pub(crate) fn record_addresses(&self) -> HashMap<NetworkAddress, RecordType> {
        HashMap::new()
    }

    #[allow(clippy::mutable_key_type)]
    pub(crate) fn record_addresses_ref(&self) -> &HashMap<Key, (NetworkAddress, RecordType)> {
        &self.empty_record_addresses
    }

    pub(crate) fn put_verified(&mut self, _r: Record, _record_type: RecordType) -> Result<()> {
        Ok(())
    }

    pub(crate) fn set_distance_range(&mut self, _distance_range: Distance) {}
}

impl RecordStore for ClientRecordStore {
    type RecordsIter<'a> = vec::IntoIter<Cow<'a, Record>>;
    type ProvidedIter<'a> = vec::IntoIter<Cow<'a, ProviderRecord>>;

    fn get(&self, _k: &Key) -> Option<Cow<'_, Record>> {
        None
    }

    fn put(&mut self, _record: Record) -> Result<()> {
        Ok(())
    }

    fn remove(&mut self, _k: &Key) {}

    fn records(&self) -> Self::RecordsIter<'_> {
        vec![].into_iter()
    }

    fn add_provider(&mut self, _record: ProviderRecord) -> Result<()> {
        Ok(())
    }

    fn providers(&self, _key: &Key) -> Vec<ProviderRecord> {
        vec![]
    }

    fn provided(&self) -> Self::ProvidedIter<'_> {
        vec![].into_iter()
    }

    fn remove_provider(&mut self, _key: &Key, _provider: &PeerId) {}
}

/// Cost calculator that increases cost nearing the maximum (MAX_RECORDS_COUNT (2048 at moment of writing)).
/// Table:
///    1 =         0.000000010
///    2 =         0.000000010
///    4 =         0.000000011
///    8 =         0.000000012
///   16 =         0.000000014
///   32 =         0.000000018
///   64 =         0.000000033
///  128 =         0.000000111
///  256 =         0.000001238
///  512 =         0.000153173
/// 1024 =         2.346196716
/// 1280 =       290.372529764
/// 1536 =     35937.398370712
/// 1792 =   4447723.077333529
/// 2048 = 550463903.051128626 (about 13% of TOTAL_SUPPLY at moment of writing)
fn calculate_cost_for_relevant_records(step: usize) -> u64 {
    assert!(
        step <= MAX_RECORDS_COUNT,
        "step must be <= MAX_RECORDS_COUNT"
    );

    // Using an exponential growth function: y = ab^x. Here, a is the starting cost and b is the growth factor.
    // We want a function that starts with a low cost and only ramps up once we get closer to the maximum.
    let a = 0.000_000_010_f64; // This is the starting cost, starting at 10 nanos.
    let b = 1.019_f64; // This is a hand-picked number; a low growth factor keeping the cost low for long.
    let y = a * b.powf(step as f64);

    (y * 1_000_000_000_f64) as u64
}

#[allow(trivial_casts)]
#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use bytes::Bytes;
    use eyre::ContextCompat;
    use libp2p::{
        core::multihash::Multihash,
        kad::{KBucketKey, RecordKey},
    };
    use quickcheck::*;
    use sn_protocol::storage::try_serialize_record;
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
            let value = match try_serialize_record(
                &(0..50).map(|_| rand::random::<u8>()).collect::<Bytes>(),
                RecordKind::Chunk,
            ) {
                Ok(value) => value.to_vec(),
                Err(err) => panic!("Cannot generate record value {:?}", err),
            };
            let record = Record {
                key: ArbitraryKey::arbitrary(g).0,
                value,
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
        let mut store = NodeRecordStore::with_config(
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

        assert!(store
            .put_verified(returned_record, RecordType::Chunk)
            .is_ok());

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
        let store_config = NodeRecordStoreConfig {
            max_records,
            ..Default::default()
        };
        let self_id = PeerId::random();
        let mut store = NodeRecordStore::with_config(self_id, store_config.clone(), None);
        let mut stored_records: Vec<RecordKey> = vec![];
        let self_address = NetworkAddress::from_peer(self_id);
        for i in 0..100 {
            let record_key = NetworkAddress::from_peer(PeerId::random()).to_record_key();
            let value = match try_serialize_record(
                &(0..50).map(|_| rand::random::<u8>()).collect::<Bytes>(),
                RecordKind::Chunk,
            ) {
                Ok(value) => value.to_vec(),
                Err(err) => panic!("Cannot generate record value {:?}", err),
            };
            let record = Record {
                key: record_key.clone(),
                value,
                publisher: None,
                expires: None,
            };
            let retained_key = if i < max_records {
                assert!(store.put_verified(record, RecordType::Chunk).is_ok());
                record_key
            } else {
                // The list is already sorted by distance, hence always shall only prune the last one
                let furthest_key = stored_records.remove(stored_records.len() - 1);
                let furthest_addr = NetworkAddress::from_record_key(&furthest_key);
                let record_addr = NetworkAddress::from_record_key(&record_key);
                let (retained_key, pruned_key) = if self_address.distance(&furthest_addr)
                    > self_address.distance(&record_addr)
                {
                    // The new entry is closer, it shall replace the existing one
                    assert!(store.put_verified(record, RecordType::Chunk).is_ok());
                    (record_key, furthest_key)
                } else {
                    // The new entry is farther away, it shall not replace the existing one
                    assert!(store.put_verified(record, RecordType::Chunk).is_err());
                    (furthest_key, record_key)
                };

                // Confirm the pruned_key got removed, looping to allow async disk ops to complete.
                let mut iteration = 0;
                while iteration < max_iterations {
                    if NodeRecordStore::read_from_disk(&pruned_key, &store_config.storage_dir)
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
                let a = NetworkAddress::from_record_key(a);
                let b = NetworkAddress::from_record_key(b);
                self_address.distance(&a).cmp(&self_address.distance(&b))
            });
        }

        Ok(())
    }

    #[tokio::test]
    #[allow(clippy::mutable_key_type)]
    async fn get_records_within_distance_range() -> eyre::Result<()> {
        let max_records = 50;

        // setup the store
        let store_config = NodeRecordStoreConfig {
            max_records,
            ..Default::default()
        };
        let self_id = PeerId::random();
        let mut store = NodeRecordStore::with_config(self_id, store_config.clone(), None);

        let mut stored_records: Vec<RecordKey> = vec![];
        let self_address = NetworkAddress::from_peer(self_id);

        // add records...
        // minus one here as if we hit max, the store will fail
        for _ in 0..max_records - 1 {
            let record_key = NetworkAddress::from_peer(PeerId::random()).to_record_key();
            let value = match try_serialize_record(
                &(0..50).map(|_| rand::random::<u8>()).collect::<Bytes>(),
                RecordKind::Chunk,
            ) {
                Ok(value) => value.to_vec(),
                Err(err) => panic!("Cannot generate record value {:?}", err),
            };
            let record = Record {
                key: record_key.clone(),
                value,
                publisher: None,
                expires: None,
            };
            // The new entry is closer, it shall replace the existing one
            assert!(store.put_verified(record, RecordType::Chunk).is_ok());

            stored_records.push(record_key);
            stored_records.sort_by(|a, b| {
                let a = NetworkAddress::from_record_key(a);
                let b = NetworkAddress::from_record_key(b);
                self_address.distance(&a).cmp(&self_address.distance(&b))
            });
        }

        // get a record halfway through the list
        let halfway_record_address = NetworkAddress::from_record_key(
            stored_records
                .get((stored_records.len() / 2) - 1)
                .wrap_err("Could not parse record store key")?,
        );
        // get the distance to this record from our local key
        let distance = self_address.distance(&halfway_record_address);

        store.set_distance_range(distance);

        let record_keys: HashSet<_> = store.records.keys().cloned().collect();

        // check that the number of records returned is correct
        assert_eq!(
            store.get_records_within_distance_range(&record_keys, distance),
            stored_records.len() / 2
        );

        Ok(())
    }
}
