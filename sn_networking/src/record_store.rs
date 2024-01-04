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
    time::{Duration, Instant},
    vec,
};
use tokio::sync::mpsc;
use xor_name::XorName;

/// Max number of records a node can store
const MAX_RECORDS_COUNT: usize = 2048;

/// Type for each record entry.
/// The `insert_time` will be reset to None after one hour.
pub(crate) type RecordStoreEntryType = HashMap<Key, (NetworkAddress, RecordType, Option<Instant>)>;

/// A `RecordStore` that stores records on disk.
pub struct NodeRecordStore {
    /// The identity of the peer owning the store.
    local_key: KBucketKey<PeerId>,
    /// The configuration of the store.
    config: NodeRecordStoreConfig,
    /// A set of keys, each corresponding to a data `Record` stored on disk.
    records: RecordStoreEntryType,
    /// Currently only used to notify the record received via network put to be validated.
    event_sender: Option<mpsc::Sender<NetworkEvent>>,
    /// Distance range specify the acceptable range of record entry.
    /// None means accept all records.
    distance_range: Option<Distance>,
    #[cfg(feature = "open-metrics")]
    /// Used to report the number of records held by the store to the metrics server.
    record_count_metric: Option<Gauge>,
    /// Counting how many times got paid
    received_payment_count: usize,
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
            received_payment_count: 0,
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
            hex_string.push_str(&format!("{byte:02x}"));
        }
        hex_string
    }

    fn read_from_disk<'a>(key: &Key, storage_dir: &Path) -> Option<Cow<'a, Record>> {
        let start = std::time::Instant::now();
        let filename = Self::key_to_hex(key);
        let file_path = storage_dir.join(&filename);

        // we should only be reading if we know the record is written to disk properly
        match fs::read(file_path) {
            Ok(value) => {
                // vdash metric (if modified please notify at https://github.com/happybeing/vdash/issues):
                info!(
                    "Retrieved record from disk! filename: {filename} after {:?}",
                    start.elapsed()
                );
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
                warn!("Record not stored (key: {r:?}). Maximum number of records reached. Current num_records: {num_records}");
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
            .map(|(_record_key, (addr, record_type, _insert_time))| {
                (addr.clone(), record_type.clone())
            })
            .collect()
    }

    /// Returns the reference to the set of `NetworkAddress::RecordKey` held by the store
    #[allow(clippy::mutable_key_type)]
    pub(crate) fn record_addresses_ref(&self) -> &RecordStoreEntryType {
        &self.records
    }

    /// Reset old entries `insert_time` to None
    pub(crate) fn reset_old_records(&mut self) {
        for val in self.records.values_mut() {
            let shall_reset = if let Some(time) = val.2 {
                time.elapsed() > Duration::from_secs(3600)
            } else {
                false
            };

            if shall_reset {
                val.2 = None;
            }
        }
    }

    /// The follow up to `put_verified`, this only registers the RecordKey
    /// in the RecordStore records set. After this it should be safe
    /// to return the record as stored.
    pub(crate) fn mark_as_stored(&mut self, key: Key, record_type: RecordType) {
        let _ = self.records.insert(
            key.clone(),
            (
                NetworkAddress::from_record_key(&key),
                record_type,
                Some(Instant::now()),
            ),
        );
    }

    /// Warning: Write's a `Record` to disk without validation
    /// Should be used in context where the `Record` is trusted
    ///
    /// The record is marked as written to disk once `mark_as_stored` is called,
    /// this avoids us returning half-written data or registering it as stored before it is.
    pub(crate) fn put_verified(&mut self, r: Record, record_type: RecordType) -> Result<()> {
        let record_key = PrettyPrintRecordKey::from(&r.key).into_owned();
        trace!("PUT a verified Record: {record_key:?}");

        self.prune_storage_if_needed_for_record(&r.key)?;

        let filename = Self::key_to_hex(&r.key);
        let file_path = self.config.storage_dir.join(&filename);

        #[cfg(feature = "open-metrics")]
        if let Some(metric) = &self.record_count_metric {
            let _ = metric.set(self.records.len() as i64);
        }

        let cloned_event_sender = self.event_sender.clone();
        tokio::spawn(async move {
            let event = match fs::write(&file_path, r.value) {
                Ok(_) => {
                    // vdash metric (if modified please notify at https://github.com/happybeing/vdash/issues):
                    info!("Wrote record {record_key:?} to disk! filename: {filename}");
                    NetworkEvent::CompletedWrite((r.key, record_type))
                }
                Err(err) => {
                    error!(
                        "Error writing record {record_key:?} filename: {filename}, error: {err:?}"
                    );
                    NetworkEvent::FailedToWrite(r.key)
                }
            };

            // This happens after the write to disk is complete
            if let Some(event_sender) = cloned_event_sender {
                if let Err(error) = event_sender.send(event).await {
                    error!("SwarmDriver failed to send event w/  {error:?}");
                }
            } else {
                error!("Record store doesn't have event_sender could not send write events for {record_key:?} {file_path:?}");
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

        let cost =
            calculate_cost_for_relevant_records(relevant_records_len, self.received_payment_count);

        // vdash metric (if modified please notify at https://github.com/happybeing/vdash/issues):
        info!("Cost is now {cost:?} for {relevant_records_len:?} records stored of {MAX_RECORDS_COUNT:?} max");
        NanoTokens::from(cost)
    }

    /// Notify the node received a payment.
    pub(crate) fn payment_received(&mut self) {
        self.received_payment_count = self.received_payment_count.saturating_add(1);
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
                            Some((_addr, RecordType::Chunk, _insert_time)) => {
                                trace!("Chunk {record_key:?} already exists.");
                                return Ok(());
                            }
                            Some((
                                _addr,
                                RecordType::NonChunk(existing_content_hash),
                                _insert_time,
                            )) => {
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
    empty_record_addresses: RecordStoreEntryType,
}

impl ClientRecordStore {
    pub(crate) fn contains(&self, _key: &Key) -> bool {
        false
    }

    pub(crate) fn record_addresses(&self) -> HashMap<NetworkAddress, RecordType> {
        HashMap::new()
    }

    #[allow(clippy::mutable_key_type)]
    pub(crate) fn record_addresses_ref(&self) -> &RecordStoreEntryType {
        &self.empty_record_addresses
    }

    pub(crate) fn put_verified(&mut self, _r: Record, _record_type: RecordType) -> Result<()> {
        Ok(())
    }

    pub(crate) fn mark_as_stored(&mut self, _r: Key, _t: RecordType) {}

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

// Using a linear growth function, and be tweaked by `received_payment_count`
// to allow nodes receiving too many replication copies can still got paid.
fn calculate_cost_for_relevant_records(step: usize, received_payment_count: usize) -> u64 {
    use std::cmp::max;

    let ori_cost = (10 * step) as u64;
    let divider = max(1, step / max(1, received_payment_count)) as u64;
    max(10, ori_cost / divider)
}

#[allow(trivial_casts)]
#[cfg(test)]
mod tests {
    use super::*;

    use crate::{close_group_majority, sort_peers_by_key, REPLICATE_RANGE};

    use bytes::Bytes;
    use eyre::ContextCompat;
    use libp2p::{
        core::multihash::Multihash,
        kad::{KBucketKey, RecordKey},
    };
    use quickcheck::*;
    use sn_protocol::storage::{try_serialize_record, ChunkAddress};
    use std::{collections::BTreeMap, time::Duration};
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
                Err(err) => panic!("Cannot generate record value {err:?}"),
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

        let returned_record_key = returned_record.key.clone();

        assert!(store
            .put_verified(returned_record, RecordType::Chunk)
            .is_ok());

        // We must also mark the record as stored (which would be triggered after the async write in nodes
        // via NetworkEvent::CompletedWrite)
        store.mark_as_stored(returned_record_key, RecordType::Chunk);

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
                Err(err) => panic!("Cannot generate record value {err:?}"),
            };
            let record = Record {
                key: record_key.clone(),
                value,
                publisher: None,
                expires: None,
            };
            let retained_key = if i < max_records {
                assert!(store.put_verified(record, RecordType::Chunk).is_ok());
                // We must also mark the record as stored (which would be triggered after the async write in nodes
                // via NetworkEvent::CompletedWrite)
                store.mark_as_stored(record_key.clone(), RecordType::Chunk);

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
                    // We must also mark the record as stored (which would be triggered after the async write in nodes
                    // via NetworkEvent::CompletedWrite)
                    store.mark_as_stored(record_key.clone(), RecordType::Chunk);

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
        let mut store = NodeRecordStore::with_config(self_id, store_config, None);

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
                Err(err) => panic!("Cannot generate record value {err:?}"),
            };
            let record = Record {
                key: record_key.clone(),
                value,
                publisher: None,
                expires: None,
            };
            // The new entry is closer, it shall replace the existing one
            assert!(store.put_verified(record, RecordType::Chunk).is_ok());
            // We must also mark the record as stored (which would be triggered after the async write in nodes
            // via NetworkEvent::CompletedWrite)
            store.mark_as_stored(record_key.clone(), RecordType::Chunk);

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

    #[test]
    fn address_distribution_sim() {
        // Map of peers and correspondent stats of `(num_of_records, Nano_earned, received_payment_count)`.
        let mut peers: HashMap<PeerId, (usize, u64, usize)> = Default::default();
        let mut peers_vec = vec![];

        let num_of_peers = 2000;
        let num_of_chunks_per_itr = 2000;

        for _ in 0..num_of_peers {
            let peer_id = PeerId::random();
            let _ = peers.insert(peer_id, (0, 0, 0));
            peers_vec.push(peer_id);
        }

        let mut iteration = 0;
        let mut total_received_payment_count = 0;

        loop {
            for _ in 0..num_of_chunks_per_itr {
                let name = xor_name::rand::random();
                let address = NetworkAddress::from_chunk_address(ChunkAddress::new(name));
                match sort_peers_by_key(&peers_vec, &address.as_kbucket_key(), REPLICATE_RANGE) {
                    Ok(peers_in_replicate_range) => {
                        let peers_in_replicate_range: Vec<PeerId> = peers_in_replicate_range
                            .iter()
                            .map(|peer_id| **peer_id)
                            .collect();
                        let peers_in_close: Vec<PeerId> = match sort_peers_by_key(
                            &peers_in_replicate_range,
                            &address.as_kbucket_key(),
                            close_group_majority(),
                        ) {
                            Ok(peers_in_close) => {
                                peers_in_close.iter().map(|peer_id| **peer_id).collect()
                            }
                            Err(err) => {
                                panic!("Cann't find close range of {name:?} with error {err:?}")
                            }
                        };

                        let payee = pick_cheapest_payee(&peers_in_close, &peers);

                        for peer in peers_in_replicate_range.iter() {
                            let entry = peers.entry(*peer).or_insert((0, 0, 0));
                            if *peer == payee {
                                let cost = calculate_cost_for_relevant_records(entry.0, entry.2);
                                entry.1 += cost;
                                entry.2 += 1;
                            }
                            entry.0 += 1;
                        }
                    }
                    Err(err) => {
                        panic!("Cann't find replicate range of {name:?} with error {err:?}")
                    }
                }
            }

            let mut received_payment_count = 0;
            let mut empty_earned_nodes = 0;

            let mut min_earned = u64::MAX;
            let mut min_store_cost = u64::MAX;
            let mut max_earned = 0;
            let mut max_store_cost = 0;

            for (_peer_id, stats) in peers.iter() {
                let cost = calculate_cost_for_relevant_records(stats.0, stats.2);
                // println!("{peer_id:?}:{stats:?} with storecost to be {cost}");
                received_payment_count += stats.2;
                if stats.1 == 0 {
                    empty_earned_nodes += 1;
                }

                if stats.1 < min_earned {
                    min_earned = stats.1;
                }
                if stats.1 > max_earned {
                    max_earned = stats.1;
                }
                if cost < min_store_cost {
                    min_store_cost = cost;
                }
                if cost > max_store_cost {
                    max_store_cost = cost;
                }
            }

            total_received_payment_count += num_of_chunks_per_itr;
            assert_eq!(total_received_payment_count, received_payment_count);

            println!("After the completion of {iteration} with {num_of_chunks_per_itr} chunks, there is still {empty_earned_nodes} nodes earned nothing");
            println!("\t\t with storecost variation of (min {min_store_cost} - max {max_store_cost}), and earned variation of (min {min_earned} - max {max_earned})");

            iteration += 1;

            // Execute for 50 iterations, which allows the test can be executed in normal CI runs.
            if iteration == 50 {
                assert_eq!(0, empty_earned_nodes);
                assert!((max_store_cost / min_store_cost) < 40);
                assert!((max_earned / min_earned) < 600);
                break;
            }
        }

        // log_chunks_distribution(&peers);
    }

    // Split nodes into groups based on its kBucketKey's leading byte of hashed_bytes.
    // This will result in 256 groups, and collect number of nodes and chunks fell into.
    #[allow(dead_code)]
    fn log_chunks_distribution(peers: &HashMap<PeerId, (usize, u64, usize)>) {
        // Using `times_of_earned` to reflect chunks hit the group.
        // This can avoid `replication counts` causing mis-understanding.
        // (number_of_nodes, times_of_earned)
        let mut distribution_map: BTreeMap<u8, (usize, usize)> = Default::default();

        for (peer_id, stats) in peers.iter() {
            let leading_byte = NetworkAddress::from_peer(*peer_id)
                .as_kbucket_key()
                .hashed_bytes()[0];
            let entry = distribution_map.entry(leading_byte).or_insert((0, 0));
            entry.0 += 1;
            entry.1 += stats.2;
        }

        for (leading_byte, stats) in distribution_map.iter() {
            println!("{leading_byte:08b}\t{}\t{} ", stats.0, stats.1)
        }
    }

    // After the completion of 0 with 2000 chunks, there is still 875 nodes earned nothing
    // After the completion of 1 with 2000 chunks, there is still 475 nodes earned nothing
    // After the completion of 2 with 2000 chunks, there is still 314 nodes earned nothing
    // After the completion of 3 with 2000 chunks, there is still 218 nodes earned nothing
    // ... ...
    // After the completion of 115 with 2000 chunks, there is still 56 nodes earned nothing
    // After the completion of 116 with 2000 chunks, there is still 56 nodes earned nothing
    // After the completion of 117 with 2000 chunks, there is still 56 nodes earned nothing
    // After the completion of 118 with 2000 chunks, there is still 56 nodes earned nothing
    // After the completion of 119 with 2000 chunks, there is still 56 nodes earned nothing
    // After the completion of 120 with 2000 chunks, there is still 56 nodes earned nothing
    // After the completion of 121 with 2000 chunks, there is still 56 nodes earned nothing
    fn pick_cheapest_payee(
        peers_in_close: &Vec<PeerId>,
        peers: &HashMap<PeerId, (usize, u64, usize)>,
    ) -> PeerId {
        let mut payee = None;
        let mut cheapest_cost = u64::MAX;

        for peer in peers_in_close {
            if let Some(stats) = peers.get(peer) {
                let store_cost = calculate_cost_for_relevant_records(stats.0, stats.2);
                if store_cost < cheapest_cost {
                    cheapest_cost = store_cost;
                    payee = Some(*peer);
                }
            } else {
                panic!("Cannot find stats of {peer:?}");
            }
        }

        if let Some(peer_id) = payee {
            peer_id
        } else {
            panic!("Cannot find cheapest payee among {peers_in_close:?}");
        }
    }
}
