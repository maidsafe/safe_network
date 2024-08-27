// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.
#![allow(clippy::mutable_key_type)] // for the Bytes in NetworkAddress

use crate::cmd::LocalSwarmCmd;
use crate::driver::MAX_PACKET_SIZE;
use crate::target_arch::{spawn, Instant};
use crate::{event::NetworkEvent, log_markers::Marker};
use crate::{send_local_swarm_cmd, CLOSE_GROUP_SIZE};
use aes_gcm_siv::{
    aead::{Aead, KeyInit, OsRng},
    Aes256GcmSiv, Nonce,
};

use itertools::Itertools;
use libp2p::{
    identity::PeerId,
    kad::{
        store::{Error, RecordStore, Result},
        KBucketDistance as Distance, KBucketKey, ProviderRecord, Record, RecordKey as Key,
    },
};
#[cfg(feature = "open-metrics")]
use prometheus_client::metrics::gauge::Gauge;
use rand::RngCore;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use serde::{Deserialize, Serialize};
use sn_protocol::{
    storage::{RecordHeader, RecordKind, RecordType},
    NetworkAddress, PrettyPrintRecordKey,
};
use sn_transfers::{NanoTokens, QuotingMetrics, TOTAL_SUPPLY};
use std::collections::VecDeque;
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
    vec,
};
use tokio::sync::mpsc;
use walkdir::{DirEntry, WalkDir};
use xor_name::XorName;

// A spend record is at the size of 4KB roughly.
// Given chunk record is maxed at size of 512KB.
// During Beta phase, it's almost one spend per chunk,
// which makes the average record size is around 256k.
// Given we are targeting small nodes use 1GB diskspace,
// this shall allow around 4K records.
const MAX_RECORDS_COUNT: usize = 4096;

/// The maximum number of records to cache in memory.
const MAX_RECORDS_CACHE_SIZE: usize = 100;

/// File name of the recorded historical quoting metrics.
const HISTORICAL_QUOTING_METRICS_FILENAME: &str = "historic_quoting_metrics";

/// A `RecordStore` that stores records on disk.
pub struct NodeRecordStore {
    /// The identity of the peer owning the store.
    local_key: KBucketKey<PeerId>,
    /// The address of the peer owning the store
    local_address: NetworkAddress,
    /// The configuration of the store.
    config: NodeRecordStoreConfig,
    /// A set of keys, each corresponding to a data `Record` stored on disk.
    records: HashMap<Key, (NetworkAddress, RecordType)>,
    /// FIFO simple cache of records to reduce read times
    records_cache: VecDeque<Record>,
    /// A map from record keys to their indices in the cache
    /// allowing for more efficient cache management
    records_cache_map: HashMap<Key, usize>,
    /// Send network events to the node layer.
    network_event_sender: mpsc::Sender<NetworkEvent>,
    /// Send cmds to the network layer. Used to interact with self in an async fashion.
    local_swarm_cmd_sender: mpsc::Sender<LocalSwarmCmd>,
    /// ilog2 distance range of responsible records
    /// AKA: how many buckets of data do we consider "close"
    /// None means accept all records.
    responsible_distance_range: Option<u32>,
    #[cfg(feature = "open-metrics")]
    /// Used to report the number of records held by the store to the metrics server.
    record_count_metric: Option<Gauge>,
    /// Counting how many times got paid
    received_payment_count: usize,
    /// Encyption cipher for the records, randomly generated at node startup
    /// Plus a 4 byte nonce starter
    encryption_details: (Aes256GcmSiv, [u8; 4]),
    /// Time that this record_store got started
    timestamp: SystemTime,
    /// Farthest record to self
    farthest_record: Option<(Key, Distance)>,
}

/// Configuration for a `DiskBackedRecordStore`.
#[derive(Debug, Clone)]
pub struct NodeRecordStoreConfig {
    /// The directory where the records are stored.
    pub storage_dir: PathBuf,
    /// The directory where the historic quote to be stored
    /// (normally to be the parent dir of the storage_dir)
    pub historic_quote_dir: PathBuf,
    /// The maximum number of records.
    pub max_records: usize,
    /// The maximum size of record values, in bytes.
    pub max_value_bytes: usize,
    /// The maximum number of records to cache in memory.
    pub records_cache_size: usize,
}

impl Default for NodeRecordStoreConfig {
    fn default() -> Self {
        let historic_quote_dir = std::env::temp_dir();
        Self {
            storage_dir: historic_quote_dir.clone(),
            historic_quote_dir,
            max_records: MAX_RECORDS_COUNT,
            max_value_bytes: MAX_PACKET_SIZE,
            records_cache_size: MAX_RECORDS_CACHE_SIZE,
        }
    }
}

/// Generate an encryption nonce for a given record key and nonce_starter bytes.
fn generate_nonce_for_record(nonce_starter: &[u8; 4], key: &Key) -> Nonce {
    let mut nonce_bytes = nonce_starter.to_vec();
    nonce_bytes.extend_from_slice(key.as_ref());
    // Ensure the final nonce is exactly 96 bits long by padding or truncating as necessary
    // https://crypto.stackexchange.com/questions/26790/how-bad-it-is-using-the-same-iv-twice-with-aes-gcm
    nonce_bytes.resize(12, 0); // 12 (u8) * 8 = 96 bits
    Nonce::from_iter(nonce_bytes)
}

#[derive(Clone, Serialize, Deserialize)]
struct HistoricQuotingMetrics {
    received_payment_count: usize,
    timestamp: SystemTime,
}

impl NodeRecordStore {
    /// If a directory for our node already exists, repopulate the records from the files in the dir
    fn update_records_from_an_existing_store(
        config: &NodeRecordStoreConfig,
        encryption_details: &(Aes256GcmSiv, [u8; 4]),
    ) -> HashMap<Key, (NetworkAddress, RecordType)> {
        let process_entry = |entry: &DirEntry| -> _ {
            let path = entry.path();
            if path.is_file() {
                debug!("Existing record found: {path:?}");
                // if we've got a file, lets try and read it
                let filename = match path.file_name().and_then(|n| n.to_str()) {
                    Some(file_name) => file_name,
                    None => {
                        // warn and remove this file as it's not a valid record
                        warn!(
                            "Found a file in the storage dir that is not a valid record: {:?}",
                            path
                        );
                        if let Err(e) = fs::remove_file(path) {
                            warn!(
                                "Failed to remove invalid record file from storage dir: {:?}",
                                e
                            );
                        }
                        return None;
                    }
                };
                // get the record key from the filename
                let key = Self::get_data_from_filename(filename)?;
                let record = match fs::read(path) {
                    Ok(bytes) => {
                        // and the stored record
                        Self::get_record_from_bytes(bytes, &key, encryption_details)?
                    }
                    Err(err) => {
                        error!("Error while reading file. filename: {filename}, error: {err:?}");
                        return None;
                    }
                };

                let record_type = match RecordHeader::is_record_of_type_chunk(&record) {
                    Ok(true) => RecordType::Chunk,
                    Ok(false) => {
                        let xorname_hash = XorName::from_content(&record.value);
                        RecordType::NonChunk(xorname_hash)
                    }
                    Err(error) => {
                        warn!("Failed to parse record type from record: {:?}", error);
                        return None;
                    }
                };

                let address = NetworkAddress::from_record_key(&key);
                info!("Existing record loaded: {path:?}");
                return Some((key, (address, record_type)));
            }
            None
        };

        info!("Attempting to repopulate records from existing store...");
        let records = WalkDir::new(&config.storage_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .collect_vec()
            .par_iter()
            .filter_map(process_entry)
            .collect();
        records
    }

    /// If quote_metrics file already exists, using the existing parameters.
    fn restore_quoting_metrics(storage_dir: &Path) -> Option<HistoricQuotingMetrics> {
        let file_path = storage_dir.join(HISTORICAL_QUOTING_METRICS_FILENAME);

        if let Ok(file) = fs::File::open(file_path) {
            if let Ok(quoting_metrics) = rmp_serde::from_read(&file) {
                return Some(quoting_metrics);
            }
        }

        None
    }

    fn flush_historic_quoting_metrics(&self) {
        let file_path = self
            .config
            .historic_quote_dir
            .join(HISTORICAL_QUOTING_METRICS_FILENAME);

        let historic_quoting_metrics = HistoricQuotingMetrics {
            received_payment_count: self.received_payment_count,
            timestamp: self.timestamp,
        };

        spawn(async move {
            if let Ok(mut file) = fs::File::create(file_path) {
                let mut serialiser = rmp_serde::encode::Serializer::new(&mut file);
                let _ = historic_quoting_metrics.serialize(&mut serialiser);
            }
        });
    }

    /// Creates a new `DiskBackedStore` with the given configuration.
    pub fn with_config(
        local_id: PeerId,
        config: NodeRecordStoreConfig,
        network_event_sender: mpsc::Sender<NetworkEvent>,
        swarm_cmd_sender: mpsc::Sender<LocalSwarmCmd>,
    ) -> Self {
        let key = Aes256GcmSiv::generate_key(&mut OsRng);
        let cipher = Aes256GcmSiv::new(&key);
        let mut nonce_starter = [0u8; 4];
        OsRng.fill_bytes(&mut nonce_starter);

        let encryption_details = (cipher, nonce_starter);

        // Recover the quoting_metrics first, as the historical file will be cleaned by
        // the later on update_records_from_an_existing_store function
        let (received_payment_count, timestamp) = if let Some(historic_quoting_metrics) =
            Self::restore_quoting_metrics(&config.historic_quote_dir)
        {
            (
                historic_quoting_metrics.received_payment_count,
                historic_quoting_metrics.timestamp,
            )
        } else {
            (0, SystemTime::now())
        };

        let records = Self::update_records_from_an_existing_store(&config, &encryption_details);

        let cache_size = config.records_cache_size;
        let mut record_store = NodeRecordStore {
            local_key: KBucketKey::from(local_id),
            local_address: NetworkAddress::from_peer(local_id),
            config,
            records,
            records_cache: VecDeque::with_capacity(cache_size),
            records_cache_map: HashMap::with_capacity(cache_size),
            network_event_sender,
            local_swarm_cmd_sender: swarm_cmd_sender,
            responsible_distance_range: None,
            #[cfg(feature = "open-metrics")]
            record_count_metric: None,
            received_payment_count,
            encryption_details,
            timestamp,
            farthest_record: None,
        };

        record_store.farthest_record = record_store.calculate_farthest();

        record_store.flush_historic_quoting_metrics();

        record_store
    }

    /// Set the record_count_metric to report the number of records stored to the metrics server
    #[cfg(feature = "open-metrics")]
    pub fn set_record_count_metric(mut self, metric: Gauge) -> Self {
        self.record_count_metric = Some(metric);
        self
    }

    /// Returns the current distance ilog2 (aka bucket) range of CLOSE_GROUP nodes.
    pub fn get_responsible_distance_range(&self) -> Option<u32> {
        self.responsible_distance_range
    }

    // Converts a Key into a Hex string.
    fn generate_filename(key: &Key) -> String {
        hex::encode(key.as_ref())
    }

    // Converts a Hex string back into a Key.
    fn get_data_from_filename(hex_str: &str) -> Option<Key> {
        match hex::decode(hex_str) {
            Ok(bytes) => Some(Key::from(bytes)),
            Err(error) => {
                error!("Error decoding hex string: {:?}", error);
                None
            }
        }
    }

    /// Upon read perform any data transformations required to return a `Record`.
    fn get_record_from_bytes<'a>(
        bytes: Vec<u8>,
        key: &Key,
        encryption_details: &(Aes256GcmSiv, [u8; 4]),
    ) -> Option<Cow<'a, Record>> {
        let mut record = Record {
            key: key.clone(),
            value: bytes,
            publisher: None,
            expires: None,
        };

        // if we're not encrypting, lets just return the record
        if !cfg!(feature = "encrypt-records") {
            return Some(Cow::Owned(record));
        }

        let (cipher, nonce_starter) = encryption_details;
        let nonce = generate_nonce_for_record(nonce_starter, key);

        match cipher.decrypt(&nonce, record.value.as_ref()) {
            Ok(value) => {
                record.value = value;
                return Some(Cow::Owned(record));
            }
            Err(error) => {
                error!("Error while decrypting record. key: {key:?}: {error:?}");
                None
            }
        }
    }

    fn read_from_disk<'a>(
        encryption_details: &(Aes256GcmSiv, [u8; 4]),
        key: &Key,
        storage_dir: &Path,
    ) -> Option<Cow<'a, Record>> {
        let start = Instant::now();
        let filename = Self::generate_filename(key);

        let file_path = storage_dir.join(&filename);

        // we should only be reading if we know the record is written to disk properly
        match fs::read(file_path) {
            Ok(bytes) => {
                // vdash metric (if modified please notify at https://github.com/happybeing/vdash/issues):
                info!(
                    "Retrieved record from disk! filename: {filename} after {:?}",
                    start.elapsed()
                );

                Self::get_record_from_bytes(bytes, key, encryption_details)
            }
            Err(err) => {
                error!("Error while reading file. filename: {filename}, error: {err:?}");
                None
            }
        }
    }

    // Returns the farthest record_key to self.
    pub fn get_farthest(&self) -> Option<Key> {
        if let Some((ref key, _distance)) = self.farthest_record {
            Some(key.clone())
        } else {
            None
        }
    }

    // Calculates the farthest record_key to self.
    fn calculate_farthest(&self) -> Option<(Key, Distance)> {
        // sort records by distance to our local key
        let mut sorted_records: Vec<_> = self.records.keys().collect();
        sorted_records.sort_by_key(|key| {
            let addr = NetworkAddress::from_record_key(key);
            self.local_address.distance(&addr)
        });

        if let Some(key) = sorted_records.last() {
            let addr = NetworkAddress::from_record_key(key);
            Some(((*key).clone(), self.local_address.distance(&addr)))
        } else {
            None
        }
    }

    /// Prune the records in the store to ensure that we free up space
    /// for the incoming record.
    /// Returns Ok if the record can be stored because it is closer to the local peer
    /// or we are not full.
    ///
    /// Err MaxRecords if we cannot store as it's farther than the farthest data we have
    fn prune_records_if_needed(&mut self, incoming_record_key: &Key) -> Result<()> {
        // we're not full, so we don't need to prune
        if self.records.len() < self.config.max_records {
            return Ok(());
        }

        if let Some((farthest_record, farthest_record_distance)) = self.farthest_record.clone() {
            // if the incoming record is farther than the farthest record, we can't store it
            if farthest_record_distance
                < self
                    .local_address
                    .distance(&NetworkAddress::from_record_key(incoming_record_key))
            {
                return Err(Error::MaxRecords);
            }

            info!(
                "Record {:?} will be pruned to free up space for new records",
                PrettyPrintRecordKey::from(&farthest_record)
            );
            self.remove(&farthest_record);
        }

        Ok(())
    }

    // When the accumulated record copies exceeds the `expotional pricing point` (max_records * 0.6)
    // those `out of range` records shall be cleaned up.
    // This is to avoid `over-quoting` during restart, when RT is not fully populated,
    // result in mis-calculation of relevant records.
    pub fn cleanup_unrelevant_records(&mut self) {
        let accumulated_records = self.records.len();
        if accumulated_records < 6 * MAX_RECORDS_COUNT / 10 {
            return;
        }

        let responsible_range = if let Some(range) = self.responsible_distance_range {
            range
        } else {
            return;
        };

        let mut removed_keys = Vec::new();
        self.records.retain(|key, _val| {
            let kbucket_key = KBucketKey::new(key.to_vec());
            let is_in_range =
                responsible_range >= self.local_key.distance(&kbucket_key).ilog2().unwrap_or(0);
            if !is_in_range {
                removed_keys.push(key.clone());
            }
            is_in_range
        });

        // Each `remove` function call will try to re-calculate furthest
        // when the key to be removed is the current furthest.
        // To avoid duplicated calculation, hence reset `furthest` first here.
        self.farthest_record = self.calculate_farthest();

        for key in removed_keys.iter() {
            // Deletion from disk will be undertaken as a spawned task,
            // hence safe to call this function repeatedly here.
            self.remove(key);
        }

        info!("Cleaned up {} unrelevant records, among the original {accumulated_records} accumulated_records",
            removed_keys.len());
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

    /// The follow up to `put_verified`, this only registers the RecordKey
    /// in the RecordStore records set. After this it should be safe
    /// to return the record as stored.
    pub(crate) fn mark_as_stored(&mut self, key: Key, record_type: RecordType) {
        let addr = NetworkAddress::from_record_key(&key);
        let _ = self
            .records
            .insert(key.clone(), (addr.clone(), record_type));

        let key_distance = self.local_address.distance(&addr);
        if let Some((_farthest_record, farthest_record_distance)) = self.farthest_record.clone() {
            if key_distance > farthest_record_distance {
                self.farthest_record = Some((key, key_distance));
            }
        } else {
            self.farthest_record = Some((key, key_distance));
        }
    }

    /// Prepare record bytes for storage
    /// If feats are enabled, this will eg, encrypt the record for storage
    fn prepare_record_bytes(
        record: Record,
        encryption_details: (Aes256GcmSiv, [u8; 4]),
    ) -> Option<Vec<u8>> {
        if !cfg!(feature = "encrypt-records") {
            return Some(record.value);
        }

        let (cipher, nonce_starter) = encryption_details;
        let nonce = generate_nonce_for_record(&nonce_starter, &record.key);

        match cipher.encrypt(&nonce, record.value.as_ref()) {
            Ok(value) => Some(value),
            Err(error) => {
                warn!(
                    "Failed to encrypt record {:?} : {error:?}",
                    PrettyPrintRecordKey::from(&record.key),
                );
                None
            }
        }
    }

    /// Warning: Write's a `Record` to disk without validation
    /// Should be used in context where the `Record` is trusted
    ///
    /// The record is marked as written to disk once `mark_as_stored` is called,
    /// this avoids us returning half-written data or registering it as stored before it is.
    pub(crate) fn put_verified(&mut self, r: Record, record_type: RecordType) -> Result<()> {
        let key = &r.key;
        let record_key = PrettyPrintRecordKey::from(&r.key).into_owned();
        debug!("PUTting a verified Record: {record_key:?}");

        // if the cache already has this record in it (eg, a conflicting spend)
        // remove it from the cache
        // self.records_cache.retain(|record| record.key != r.key);
        // Remove from cache if it already exists
        if let Some(&index) = self.records_cache_map.get(key) {
            if let Some(existing_record) = self.records_cache.remove(index) {
                if existing_record.value == r.value {
                    // we actually just want to keep what we have, and can assume it's been stored properly.

                    // so we put it back in the cache
                    self.records_cache.insert(index, existing_record);
                    // and exit early.
                    return Ok(());
                }
            }
            self.update_cache_indices(index);
        }

        // Store in the FIFO records cache, removing the oldest if needed
        if self.records_cache.len() >= self.config.records_cache_size {
            if let Some(old_record) = self.records_cache.pop_front() {
                self.records_cache_map.remove(&old_record.key);
            }
        }

        // Push the new record to the back of the cache
        self.records_cache.push_back(r.clone());
        self.records_cache_map
            .insert(key.clone(), self.records_cache.len() - 1);

        self.prune_records_if_needed(key)?;

        let filename = Self::generate_filename(key);
        let file_path = self.config.storage_dir.join(&filename);

        #[cfg(feature = "open-metrics")]
        if let Some(metric) = &self.record_count_metric {
            let _ = metric.set(self.records.len() as i64);
        }

        let encryption_details = self.encryption_details.clone();
        let cloned_cmd_sender = self.local_swarm_cmd_sender.clone();

        let record_key2 = record_key.clone();
        spawn(async move {
            let key = r.key.clone();
            if let Some(bytes) = Self::prepare_record_bytes(r, encryption_details) {
                let cmd = match fs::write(&file_path, bytes) {
                    Ok(_) => {
                        // vdash metric (if modified please notify at https://github.com/happybeing/vdash/issues):
                        info!("Wrote record {record_key2:?} to disk! filename: {filename}");

                        LocalSwarmCmd::AddLocalRecordAsStored { key, record_type }
                    }
                    Err(err) => {
                        error!(
                        "Error writing record {record_key2:?} filename: {filename}, error: {err:?}"
                    );
                        LocalSwarmCmd::RemoveFailedLocalRecord { key }
                    }
                };

                send_local_swarm_cmd(cloned_cmd_sender, cmd);
            }
        });

        Ok(())
    }

    /// Update the cache indices after removing an element
    fn update_cache_indices(&mut self, start_index: usize) {
        for index in start_index..self.records_cache.len() {
            if let Some(record) = self.records_cache.get(index) {
                self.records_cache_map.insert(record.key.clone(), index);
            }
        }
    }

    /// Calculate the cost to store data for our current store state
    #[allow(clippy::mutable_key_type)]
    pub(crate) fn store_cost(&self, key: &Key) -> (NanoTokens, QuotingMetrics) {
        let records_stored = self.records.len();
        let record_keys_as_hashset: HashSet<&Key> = self.records.keys().collect();

        let live_time = if let Ok(elapsed) = self.timestamp.elapsed() {
            elapsed.as_secs()
        } else {
            0
        };

        let mut quoting_metrics = QuotingMetrics {
            close_records_stored: records_stored,
            max_records: self.config.max_records,
            received_payment_count: self.received_payment_count,
            live_time,
        };

        if let Some(distance_range) = self.responsible_distance_range {
            let relevant_records =
                self.get_records_within_distance_range(record_keys_as_hashset, distance_range);

            quoting_metrics.close_records_stored = relevant_records;
        } else {
            info!("Basing cost of _total_ records stored.");
        };

        let cost = if self.contains(key) {
            0
        } else {
            calculate_cost_for_records(&quoting_metrics)
        };
        // vdash metric (if modified please notify at https://github.com/happybeing/vdash/issues):
        info!("Cost is now {cost:?} for quoting_metrics {quoting_metrics:?}");

        (NanoTokens::from(cost), quoting_metrics)
    }

    /// Notify the node received a payment.
    pub(crate) fn payment_received(&mut self) {
        self.received_payment_count = self.received_payment_count.saturating_add(1);

        self.flush_historic_quoting_metrics();
    }

    /// Calculate how many records are stored within a distance range
    #[allow(clippy::mutable_key_type)]
    pub fn get_records_within_distance_range(
        &self,
        records: HashSet<&Key>,
        distance_range: u32,
    ) -> usize {
        debug!(
            "Total record count is {:?}. Distance is: {distance_range:?}",
            self.records.len()
        );

        let relevant_records_len = records
            .iter()
            .filter(|key| {
                let kbucket_key = KBucketKey::new(key.to_vec());
                distance_range >= self.local_key.distance(&kbucket_key).ilog2().unwrap_or(0)
            })
            .count();

        Marker::CloseRecordsLen(relevant_records_len).log();
        relevant_records_len
    }

    /// Setup the distance range.
    pub(crate) fn set_responsible_distance_range(&mut self, farthest_responsible_bucket: u32) {
        self.responsible_distance_range = Some(farthest_responsible_bucket);
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

        let cached_record = self.records_cache.iter().find(|r| r.key == *k);
        // first return from FIFO cache if existing there
        if let Some(record) = cached_record {
            return Some(Cow::Borrowed(record));
        }

        if !self.records.contains_key(k) {
            debug!("Record not found locally: {key:?}");
            return None;
        }

        debug!("GET request for Record key: {key}");

        Self::read_from_disk(&self.encryption_details, k, &self.config.storage_dir)
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
                        debug!("Record {record_key:?} with payment shall always be processed.");
                    }
                    _ => {
                        // Chunk with existing key do not to be stored again.
                        // `Spend` or `Register` with same content_hash do not to be stored again,
                        // otherwise shall be passed further to allow
                        // double spend to be detected or register op update.
                        match self.records.get(&record.key) {
                            Some((_addr, RecordType::Chunk)) => {
                                debug!("Chunk {record_key:?} already exists.");
                                return Ok(());
                            }
                            Some((_addr, RecordType::NonChunk(existing_content_hash))) => {
                                let content_hash = XorName::from_content(&record.value);
                                if content_hash == *existing_content_hash {
                                    debug!("A non-chunk record {record_key:?} with same content_hash {content_hash:?} already exists.");
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

        debug!("Unverified Record {record_key:?} try to validate and store");
        let event_sender = self.network_event_sender.clone();
        // push the event off thread so as to be non-blocking
        let _handle = spawn(async move {
            if let Err(error) = event_sender
                .send(NetworkEvent::UnverifiedRecord(record))
                .await
            {
                error!("SwarmDriver failed to send event: {}", error);
            }
        });

        Ok(())
    }

    fn remove(&mut self, k: &Key) {
        let _ = self.records.remove(k);
        self.records_cache.retain(|r| r.key != *k);

        #[cfg(feature = "open-metrics")]
        if let Some(metric) = &self.record_count_metric {
            let _ = metric.set(self.records.len() as i64);
        }

        if let Some((farthest_record, _)) = self.farthest_record.clone() {
            if farthest_record == *k {
                self.farthest_record = self.calculate_farthest();
            }
        }

        let filename = Self::generate_filename(k);
        let file_path = self.config.storage_dir.join(&filename);

        let _handle = spawn(async move {
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

    pub(crate) fn mark_as_stored(&mut self, _r: Key, _t: RecordType) {}
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

// Using a linear growth function, and be tweaked by `received_payment_count`,
// `max_records` and `live_time`(in seconds),
// to allow nodes receiving too many replication copies can still got paid,
// and gives an exponential pricing curve when storage reaches high.
// and give extra reward (lower the quoting price to gain a better chance) to long lived nodes.
pub fn calculate_cost_for_records(quoting_metrics: &QuotingMetrics) -> u64 {
    use std::cmp::{max, min};

    let records_stored = quoting_metrics.close_records_stored;
    let received_payment_count = quoting_metrics.received_payment_count;
    let max_records = quoting_metrics.max_records;
    let live_time = quoting_metrics.live_time;

    let ori_cost = (10 * records_stored) as u64;
    let divider = max(1, records_stored / max(1, received_payment_count)) as u64;

    // Gaining one step for every day that staying in the network
    let reward_steps: u64 = live_time / (24 * 3600);
    let base_multiplier = 1.1_f32;
    let rewarder = max(1, base_multiplier.powf(reward_steps as f32) as u64);

    // Fine tuning here helps to get a desired curve:
    // 1, Close to the max supply (4.3E+18) when stored records reaching full.
    // 2, Charging around `token`s near the situation of 80% storage reached.
    //
    // 1.02.powf(1638) = 1.25E+14 => charge_at_full = 10 * MAX_RECORDS * 1.25E+14 = 5.4E+18
    // 1.02.powf(820) = 1.1E+7 => charge_at_80_percent = 10 * 0.8 * MAX_RECORDS * 1.1E+7 = 3.6E+11 (360 tokens)
    let base_multiplier = 1.0225_f32;

    // Given currently the max_records is set at MAX_RECORDS,
    // hence setting the multiplier trigger at 60% of the max_records
    let exponential_pricing_trigger = 6 * max_records / 10;

    let multiplier = max(
        1,
        base_multiplier.powf(records_stored.saturating_sub(exponential_pricing_trigger) as f32)
            as u64,
    );

    let charge = max(10, ori_cost.saturating_mul(multiplier) / divider / rewarder);
    // Deploy an upper cap safe_guard to the store_cost
    min(TOTAL_SUPPLY / CLOSE_GROUP_SIZE as u64, charge)
}

#[allow(trivial_casts)]
#[cfg(test)]
mod tests {

    use super::*;
    use crate::{close_group_majority, sort_peers_by_key, REPLICATION_PEERS_COUNT};
    use bytes::Bytes;
    use eyre::ContextCompat;
    use libp2p::{core::multihash::Multihash, kad::RecordKey};
    use quickcheck::*;
    use sn_protocol::storage::{try_serialize_record, ChunkAddress};
    use std::collections::BTreeMap;
    use tokio::runtime::Runtime;
    use tokio::time::{sleep, Duration};

    const MULITHASH_CODE: u64 = 0x12;

    #[derive(Clone, Debug)]
    struct ArbitraryKey(Key);
    #[derive(Clone, Debug)]
    struct ArbitraryRecord(Record);

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

    #[test]
    fn test_calculate_max_cost_for_records() {
        let sut = calculate_cost_for_records(&QuotingMetrics {
            close_records_stored: MAX_RECORDS_COUNT + 1,
            max_records: MAX_RECORDS_COUNT,
            received_payment_count: MAX_RECORDS_COUNT + 1,
            live_time: 1,
        });
        assert_eq!(sut, TOTAL_SUPPLY / CLOSE_GROUP_SIZE as u64);
    }

    #[test]
    fn test_calculate_50_percent_cost_for_records() {
        let percent = MAX_RECORDS_COUNT * 50 / 100;
        let sut = calculate_cost_for_records(&QuotingMetrics {
            close_records_stored: percent,
            max_records: MAX_RECORDS_COUNT,
            received_payment_count: percent,
            live_time: 1,
        });
        // at this point we should be at max cost
        assert_eq!(sut, 20480);
    }
    #[test]
    fn test_calculate_60_percent_cost_for_records() {
        let percent = MAX_RECORDS_COUNT * 60 / 100;
        let sut = calculate_cost_for_records(&QuotingMetrics {
            close_records_stored: percent,
            max_records: MAX_RECORDS_COUNT,
            received_payment_count: percent,
            live_time: 1,
        });
        // at this point we should be at max cost
        assert_eq!(sut, 24570);
    }

    #[test]
    fn test_calculate_65_percent_cost_for_records() {
        let percent = MAX_RECORDS_COUNT * 65 / 100;
        let sut = calculate_cost_for_records(&QuotingMetrics {
            close_records_stored: percent,
            max_records: MAX_RECORDS_COUNT,
            received_payment_count: percent,
            live_time: 1,
        });
        // at this point we should be at max cost
        assert_eq!(sut, 2528900);
    }

    #[test]
    fn test_calculate_70_percent_cost_for_records() {
        let percent = MAX_RECORDS_COUNT * 70 / 100;
        let sut = calculate_cost_for_records(&QuotingMetrics {
            close_records_stored: percent,
            max_records: MAX_RECORDS_COUNT,
            received_payment_count: percent,
            live_time: 1,
        });
        // at this point we should be at max cost
        assert_eq!(sut, 262645870);
    }

    #[test]
    fn test_calculate_80_percent_cost_for_records() {
        let percent = MAX_RECORDS_COUNT * 80 / 100;
        let sut = calculate_cost_for_records(&QuotingMetrics {
            close_records_stored: percent,
            max_records: MAX_RECORDS_COUNT,
            received_payment_count: percent,
            live_time: 1,
        });
        // at this point we should be at max cost
        assert_eq!(sut, 2689140767040);
    }

    #[test]
    fn test_calculate_90_percent_cost_for_records() {
        let percent = MAX_RECORDS_COUNT * 90 / 100;
        let sut = calculate_cost_for_records(&QuotingMetrics {
            close_records_stored: percent,
            max_records: MAX_RECORDS_COUNT,
            received_payment_count: percent,
            live_time: 1,
        });
        // at this point we should be at max cost
        assert_eq!(sut, 27719885856440320);
    }

    #[test]
    fn test_calculate_min_cost_for_records() {
        let sut = calculate_cost_for_records(&QuotingMetrics {
            close_records_stored: 0,
            max_records: MAX_RECORDS_COUNT,
            received_payment_count: 0,
            live_time: 1,
        });
        assert_eq!(sut, 10);
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
        let (swarm_cmd_sender, _) = mpsc::channel(1);

        let mut store = NodeRecordStore::with_config(
            PeerId::random(),
            Default::default(),
            network_event_sender,
            swarm_cmd_sender,
        );

        let store_cost_before = store.store_cost(&r.key);
        // An initial unverified put should not write to disk
        assert!(store.put(r.clone()).is_ok());
        assert!(store.get(&r.key).is_none());
        // Store cost should not change if no PUT has been added
        assert_eq!(
            store.store_cost(&r.key).0,
            store_cost_before.0,
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
            sleep(Duration::from_millis(100)).await;
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
        // lower max records for faster testing
        let max_records = 50;

        let temp_dir = std::env::temp_dir();
        let unique_dir_name = uuid::Uuid::new_v4().to_string();
        let storage_dir = temp_dir.join(unique_dir_name);
        fs::create_dir_all(&storage_dir).expect("Failed to create directory");

        // Set the config::max_record to be 50, then generate 100 records
        // On storing the 51st to 100th record,
        // check there is an expected pruning behaviour got carried out.
        let store_config = NodeRecordStoreConfig {
            max_records,
            storage_dir,
            ..Default::default()
        };
        let self_id = PeerId::random();
        let (network_event_sender, _) = mpsc::channel(1);
        let (swarm_cmd_sender, _) = mpsc::channel(1);

        let mut store = NodeRecordStore::with_config(
            self_id,
            store_config.clone(),
            network_event_sender,
            swarm_cmd_sender,
        );
        // keep track of everything ever stored, to check missing at the end are further away
        let mut stored_records_at_some_point: Vec<RecordKey> = vec![];
        let self_address = NetworkAddress::from_peer(self_id);

        // keep track of fails to assert they're further than stored
        let mut failed_records = vec![];

        // try and put an excess of records
        for _ in 0..max_records * 2 {
            // println!("i: {i}");
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

            // Will be stored anyway.
            let succeeded = store.put_verified(record, RecordType::Chunk).is_ok();

            if !succeeded {
                failed_records.push(record_key.clone());
                println!("failed {:?}", PrettyPrintRecordKey::from(&record_key));
            } else {
                // We must also mark the record as stored (which would be triggered
                // after the async write in nodes via NetworkEvent::CompletedWrite)
                store.mark_as_stored(record_key.clone(), RecordType::Chunk);

                println!("success sotred len: {:?} ", store.record_addresses().len());
                stored_records_at_some_point.push(record_key.clone());
                if stored_records_at_some_point.len() <= max_records {
                    assert!(succeeded);
                }
                // loop over max_iterations times to ensure async disk write had time to complete.
                let mut iteration = 0;
                while iteration < max_iterations {
                    if store.get(&record_key).is_some() {
                        break;
                    }
                    sleep(Duration::from_millis(100)).await;
                    iteration += 1;
                }
                if iteration == max_iterations {
                    panic!("record_store prune test failed with stored record {record_key:?} can't be read back");
                }
            }
        }

        let stored_data_at_end = store.record_addresses();
        assert!(
            stored_data_at_end.len() == max_records,
            "Stored records ({:?}) should be max_records, {max_records:?}",
            stored_data_at_end.len(),
        );

        // now assert that we've stored at _least_ max records (likely many more over the liftime of the store)
        assert!(
            stored_records_at_some_point.len() >= max_records,
            "we should have stored ata least max over time"
        );

        // now all failed records should be farther than the farthest stored record
        let mut sorted_stored_data = stored_data_at_end.iter().collect_vec();

        sorted_stored_data
            .sort_by(|(a, _), (b, _)| self_address.distance(a).cmp(&self_address.distance(b)));

        // next assert that all records stored are closer than the next closest of the failed records
        if let Some((most_distant_data, _)) = sorted_stored_data.last() {
            for failed_record in failed_records {
                let failed_data = NetworkAddress::from_record_key(&failed_record);
                assert!(
                    self_address.distance(&failed_data) > self_address.distance(most_distant_data),
                    "failed record {failed_data:?} should be farther than the farthest stored record {most_distant_data:?}"
                );
            }

            // now for any stored data. It either shoudl still be stored OR further away than `most_distant_data`
            for data in stored_records_at_some_point {
                let data_addr = NetworkAddress::from_record_key(&data);
                if !sorted_stored_data.contains(&(&data_addr, &RecordType::Chunk)) {
                    assert!(
                        self_address.distance(&data_addr)
                            > self_address.distance(most_distant_data),
                        "stored record should be farther than the farthest stored record"
                    );
                }
            }
        }

        Ok(())
    }

    #[tokio::test]
    #[allow(clippy::mutable_key_type)]
    async fn get_records_within_bucket_range() -> eyre::Result<()> {
        let max_records = 50;

        let temp_dir = std::env::temp_dir();
        let unique_dir_name = uuid::Uuid::new_v4().to_string();
        let storage_dir = temp_dir.join(unique_dir_name);

        // setup the store
        let store_config = NodeRecordStoreConfig {
            max_records,
            storage_dir,
            ..Default::default()
        };
        let self_id = PeerId::random();
        let (network_event_sender, _) = mpsc::channel(1);
        let (swarm_cmd_sender, _) = mpsc::channel(1);
        let mut store = NodeRecordStore::with_config(
            self_id,
            store_config,
            network_event_sender,
            swarm_cmd_sender,
        );

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
        let distance = self_address
            .distance(&halfway_record_address)
            .ilog2()
            .unwrap_or(0);

        // must be plus one bucket from the halfway record
        store.set_responsible_distance_range(distance);

        let record_keys = store.records.keys().collect();

        // check that the number of records returned is larger than half our records
        // (ie, that we cover _at least_ all the records within our distance range)
        assert!(
            store.get_records_within_distance_range(record_keys, distance)
                >= stored_records.len() / 2
        );

        Ok(())
    }

    #[tokio::test]
    async fn historic_quoting_metrics() -> Result<()> {
        let temp_dir = std::env::temp_dir();
        let unique_dir_name = uuid::Uuid::new_v4().to_string();
        let storage_dir = temp_dir.join(unique_dir_name);
        fs::create_dir_all(&storage_dir).expect("Failed to create directory");
        let historic_quote_dir = storage_dir.clone();

        let store_config = NodeRecordStoreConfig {
            storage_dir,
            historic_quote_dir,
            ..Default::default()
        };
        let self_id = PeerId::random();
        let (network_event_sender, _) = mpsc::channel(1);
        let (swarm_cmd_sender, _) = mpsc::channel(1);

        let mut store = NodeRecordStore::with_config(
            self_id,
            store_config.clone(),
            network_event_sender.clone(),
            swarm_cmd_sender.clone(),
        );

        store.payment_received();

        // Wait for a while to allow the file written to disk.
        sleep(Duration::from_millis(5000)).await;

        let new_store = NodeRecordStore::with_config(
            self_id,
            store_config,
            network_event_sender,
            swarm_cmd_sender,
        );

        assert_eq!(1, new_store.received_payment_count);
        assert_eq!(store.timestamp, new_store.timestamp);

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
                match sort_peers_by_key(
                    &peers_vec,
                    &address.as_kbucket_key(),
                    REPLICATION_PEERS_COUNT,
                ) {
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
                                panic!("Can't find close range of {name:?} with error {err:?}")
                            }
                        };

                        let payee = pick_cheapest_payee(&peers_in_close, &peers);

                        for peer in peers_in_replicate_range.iter() {
                            let (close_records_stored, nanos_earnt, received_payment_count) =
                                peers.entry(*peer).or_insert((0, 0, 0));
                            if *peer == payee {
                                let cost = calculate_cost_for_records(&QuotingMetrics {
                                    close_records_stored: *close_records_stored,
                                    max_records: MAX_RECORDS_COUNT,
                                    received_payment_count: *received_payment_count,
                                    live_time: 0,
                                });
                                *nanos_earnt += cost;
                                *received_payment_count += 1;
                            }
                            *close_records_stored += 1;
                        }
                    }
                    Err(err) => {
                        panic!("Can't find replicate range of {name:?} with error {err:?}")
                    }
                }
            }

            let mut received_payment_count = 0;
            let mut empty_earned_nodes = 0;

            let mut min_earned = u64::MAX;
            let mut min_store_cost = u64::MAX;
            let mut max_earned = 0;
            let mut max_store_cost = 0;

            for (_peer_id, (close_records_stored, nanos_earnt, times_paid)) in peers.iter() {
                let cost = calculate_cost_for_records(&QuotingMetrics {
                    close_records_stored: *close_records_stored,
                    max_records: MAX_RECORDS_COUNT,
                    received_payment_count: *times_paid,
                    live_time: 0,
                });
                // println!("{peer_id:?}:{stats:?} with storecost to be {cost}");
                received_payment_count += times_paid;
                if *nanos_earnt == 0 {
                    empty_earned_nodes += 1;
                }

                if *nanos_earnt < min_earned {
                    min_earned = *nanos_earnt;
                }
                if *nanos_earnt > max_earned {
                    max_earned = *nanos_earnt;
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
                assert_eq!(0, empty_earned_nodes, "every node has earnt _something_");
                assert!(
                    (max_store_cost / min_store_cost) < 100,
                    "store cost is balanced"
                );
                assert!(
                    (max_earned / min_earned) < 1000,
                    "earning distribution is well balanced"
                );
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
                let store_cost = calculate_cost_for_records(&QuotingMetrics {
                    close_records_stored: stats.0,
                    max_records: MAX_RECORDS_COUNT,
                    received_payment_count: stats.2,
                    live_time: 0,
                });
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
