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
use crate::send_local_swarm_cmd;
use crate::target_arch::{spawn, Instant};
use crate::{event::NetworkEvent, log_markers::Marker};
use aes_gcm_siv::{
    aead::{Aead, KeyInit, OsRng},
    Aes256GcmSiv, Nonce,
};

use itertools::Itertools;
use libp2p::{
    identity::PeerId,
    kad::{
        store::{Error, RecordStore, Result},
        KBucketDistance as Distance, ProviderRecord, Record, RecordKey as Key,
    },
};
#[cfg(feature = "open-metrics")]
use prometheus_client::metrics::gauge::Gauge;
use rand::RngCore;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use serde::{Deserialize, Serialize};
use sn_evm::{AttoTokens, QuotingMetrics};
use sn_protocol::{
    storage::{RecordHeader, RecordKind, RecordType},
    NetworkAddress, PrettyPrintRecordKey,
};
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
// Given chunk record is maxed at size of 4MB.
// During Beta phase, it's almost one spend per chunk,
// which makes the average record size is around 2MB.
// Given we are targeting node size to be 32GB,
// this shall allow around 16K records.
const MAX_RECORDS_COUNT: usize = 16 * 1024;

/// The maximum number of records to cache in memory.
const MAX_RECORDS_CACHE_SIZE: usize = 25;

/// File name of the recorded historical quoting metrics.
const HISTORICAL_QUOTING_METRICS_FILENAME: &str = "historic_quoting_metrics";

/// Max store cost for a chunk.
const MAX_STORE_COST: u64 = 1_000_000;

// Min store cost for a chunk.
const MIN_STORE_COST: u64 = 1;

/// FIFO simple cache of records to reduce read times
struct RecordCache {
    records_cache: HashMap<Key, (Record, SystemTime)>,
    cache_size: usize,
}

impl RecordCache {
    fn new(cache_size: usize) -> Self {
        RecordCache {
            records_cache: HashMap::new(),
            cache_size,
        }
    }

    fn remove(&mut self, key: &Key) -> Option<(Record, SystemTime)> {
        self.records_cache.remove(key)
    }

    fn get(&self, key: &Key) -> Option<&(Record, SystemTime)> {
        self.records_cache.get(key)
    }

    fn push_back(&mut self, key: Key, record: Record) {
        self.free_up_space();

        let _ = self.records_cache.insert(key, (record, SystemTime::now()));
    }

    fn free_up_space(&mut self) {
        while self.records_cache.len() >= self.cache_size {
            self.remove_oldest_entry()
        }
    }

    fn remove_oldest_entry(&mut self) {
        let mut oldest_timestamp = SystemTime::now();

        for (_record, timestamp) in self.records_cache.values() {
            if *timestamp < oldest_timestamp {
                oldest_timestamp = *timestamp;
            }
        }

        self.records_cache
            .retain(|_key, (_record, timestamp)| *timestamp != oldest_timestamp);
    }
}

/// A `RecordStore` that stores records on disk.
pub struct NodeRecordStore {
    /// The address of the peer owning the store
    local_address: NetworkAddress,
    /// The configuration of the store.
    config: NodeRecordStoreConfig,
    /// Main records store remains unchanged for compatibility
    records: HashMap<Key, (NetworkAddress, RecordType)>,
    /// Additional index organizing records by distance bucket
    records_by_bucket: HashMap<u32, HashSet<Key>>,
    /// FIFO simple cache of records to reduce read times
    records_cache: RecordCache,
    /// Send network events to the node layer.
    network_event_sender: mpsc::Sender<NetworkEvent>,
    /// Send cmds to the network layer. Used to interact with self in an async fashion.
    local_swarm_cmd_sender: mpsc::Sender<LocalSwarmCmd>,
    /// ilog2 distance range of responsible records
    /// AKA: how many buckets of data do we consider "close"
    /// None means accept all records.
    responsible_distance_range: Option<Distance>,
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
            local_address: NetworkAddress::from_peer(local_id),
            config,
            records,
            records_by_bucket: HashMap::new(),
            records_cache: RecordCache::new(cache_size),
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
    pub fn cleanup_irrelevant_records(&mut self) {
        let accumulated_records = self.records.len();
        if accumulated_records < MAX_RECORDS_COUNT * 6 / 10 {
            return;
        }

        let responsible_range = if let Some(range) = self.responsible_distance_range {
            range
        } else {
            return;
        };

        let max_bucket = responsible_range.ilog2().unwrap_or_default();

        // Collect keys to remove from buckets beyond our range
        let keys_to_remove: Vec<Key> = self
            .records_by_bucket
            .iter()
            .filter(|(&bucket, _)| bucket > max_bucket)
            .flat_map(|(_, keys)| keys.iter().cloned())
            .collect();

        let keys_to_remove_len = keys_to_remove.len();

        // Remove collected keys
        for key in keys_to_remove {
            self.remove(&key);
        }

        info!("Cleaned up {} unrelevant records, among the original {accumulated_records} accumulated_records",
        keys_to_remove_len);
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
    pub(crate) fn record_addresses_ref(&self) -> &HashMap<Key, (NetworkAddress, RecordType)> {
        &self.records
    }

    /// The follow up to `put_verified`, this only registers the RecordKey
    /// in the RecordStore records set. After this it should be safe
    /// to return the record as stored.
    pub(crate) fn mark_as_stored(&mut self, key: Key, record_type: RecordType) {
        let addr = NetworkAddress::from_record_key(&key);
        let distance = self.local_address.distance(&addr);
        let bucket = distance.ilog2().unwrap_or_default();

        // Update main records store
        self.records
            .insert(key.clone(), (addr.clone(), record_type));

        // Update bucket index
        self.records_by_bucket
            .entry(bucket)
            .or_default()
            .insert(key.clone());

        // Update farthest record if needed (unchanged)
        if let Some((_farthest_record, farthest_record_distance)) = self.farthest_record.clone() {
            if distance > farthest_record_distance {
                self.farthest_record = Some((key, distance));
            }
        } else {
            self.farthest_record = Some((key, distance));
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

        // if cache already has the record :
        //   * if with same content, do nothing and return early
        //   * if with different content, remove the existing one
        if let Some((existing_record, _timestamp)) = self.records_cache.remove(key) {
            if existing_record.value == r.value {
                // we actually just want to keep what we have, and can assume it's been stored properly.

                // so we put it back in the cache
                self.records_cache.push_back(key.clone(), existing_record);
                // and exit early.
                return Ok(());
            }
        }

        // Store the new record to the cache
        self.records_cache.push_back(key.clone(), r.clone());

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

    /// Calculate the cost to store data for our current store state
    pub(crate) fn store_cost(&self, key: &Key) -> (AttoTokens, QuotingMetrics) {
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
            calculate_cost_for_records(quoting_metrics.close_records_stored)
        };
        // vdash metric (if modified please notify at https://github.com/happybeing/vdash/issues):
        info!("Cost is now {cost:?} for quoting_metrics {quoting_metrics:?}");

        (AttoTokens::from_u64(cost), quoting_metrics)
    }

    /// Notify the node received a payment.
    pub(crate) fn payment_received(&mut self) {
        self.received_payment_count = self.received_payment_count.saturating_add(1);

        self.flush_historic_quoting_metrics();
    }

    /// Calculate how many records are stored within a distance range
    pub fn get_records_within_distance_range(
        &self,
        _records: HashSet<&Key>,
        max_distance: Distance,
    ) -> usize {
        let max_bucket = max_distance.ilog2().unwrap_or_default();

        let within_range = self
            .records_by_bucket
            .iter()
            .filter(|(&bucket, _)| bucket <= max_bucket)
            .map(|(_, keys)| keys.len())
            .sum();

        Marker::CloseRecordsLen(within_range).log();

        within_range
    }

    /// Setup the distance range.
    pub(crate) fn set_responsible_distance_range(&mut self, farthest_distance: Distance) {
        self.responsible_distance_range = Some(farthest_distance);
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

        let cached_record = self.records_cache.get(k);
        // first return from FIFO cache if existing there
        if let Some((record, _timestamp)) = cached_record {
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
        let record_key = PrettyPrintRecordKey::from(&record.key);

        if record.value.len() >= self.config.max_value_bytes {
            warn!(
                "Record {record_key:?} not stored. Value too large: {} bytes",
                record.value.len()
            );
            return Err(Error::ValueTooLarge);
        }

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
        // Remove from main store
        if let Some((addr, _)) = self.records.remove(k) {
            // Remove from bucket index
            let bucket = self
                .local_address
                .distance(&addr)
                .ilog2()
                .unwrap_or_default();
            if let Some(bucket_keys) = self.records_by_bucket.get_mut(&bucket) {
                bucket_keys.remove(k);
                // Clean up empty buckets
                if bucket_keys.is_empty() {
                    self.records_by_bucket.remove(&bucket);
                }
            }
        }

        self.records_cache.remove(k);

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

// Using a linear growth function tweaked by `max_records`,
// and gives an exponential pricing curve when storage reaches high.
// and give extra reward (lower the quoting price to gain a better chance) to long lived nodes.
pub fn calculate_cost_for_records(records_stored: usize) -> u64 {
    use std::cmp::{max, min};

    let max_records = MAX_RECORDS_COUNT;

    let ori_cost = positive_input_0_1_sigmoid(records_stored as f64 / max_records as f64)
        * MAX_STORE_COST as f64;

    // Deploy a lower cap safe_guard to the store_cost
    let charge = max(MIN_STORE_COST, ori_cost as u64);
    // Deploy an upper cap safe_guard to the store_cost
    min(MAX_STORE_COST, charge)
}

fn positive_input_0_1_sigmoid(x: f64) -> f64 {
    1.0 / (1.0 + (-30.0 * (x - 0.5)).exp())
}

#[expect(trivial_casts)]
#[cfg(test)]
mod tests {

    use crate::get_fees_from_store_cost_responses;

    use super::*;
    use bls::SecretKey;
    use xor_name::XorName;

    use bytes::Bytes;
    use eyre::{bail, ContextCompat};
    use libp2p::kad::K_VALUE;
    use libp2p::{core::multihash::Multihash, kad::RecordKey};
    use quickcheck::*;
    use sn_evm::utils::dummy_address;
    use sn_evm::{PaymentQuote, RewardsAddress};
    use sn_protocol::storage::{
        try_deserialize_record, try_serialize_record, Chunk, ChunkAddress, Scratchpad,
    };
    use std::collections::BTreeMap;
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
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
        let sut = calculate_cost_for_records(MAX_RECORDS_COUNT + 1);
        assert_eq!(sut, MAX_STORE_COST - 1);
    }

    #[test]
    fn test_calculate_50_percent_cost_for_records() {
        let percent = MAX_RECORDS_COUNT * 50 / 100;
        let sut = calculate_cost_for_records(percent);

        // at this point we should be at max cost
        assert_eq!(sut, 500000);
    }
    #[test]
    fn test_calculate_60_percent_cost_for_records() {
        let percent = MAX_RECORDS_COUNT * 60 / 100;
        let sut = calculate_cost_for_records(percent);

        // at this point we should be at max cost
        assert_eq!(sut, 952541);
    }

    #[test]
    fn test_calculate_65_percent_cost_for_records() {
        let percent = MAX_RECORDS_COUNT * 65 / 100;
        let sut = calculate_cost_for_records(percent);

        // at this point we should be at max cost
        assert_eq!(sut, 989001);
    }

    #[test]
    fn test_calculate_70_percent_cost_for_records() {
        let percent = MAX_RECORDS_COUNT * 70 / 100;
        let sut = calculate_cost_for_records(percent);

        // at this point we should be at max cost
        assert_eq!(sut, 997523);
    }

    #[test]
    fn test_calculate_80_percent_cost_for_records() {
        let percent = MAX_RECORDS_COUNT * 80 / 100;
        let sut = calculate_cost_for_records(percent);

        // at this point we should be at max cost
        assert_eq!(sut, 999876);
    }

    #[test]
    fn test_calculate_90_percent_cost_for_records() {
        let percent = MAX_RECORDS_COUNT * 90 / 100;
        let sut = calculate_cost_for_records(percent);
        // at this point we should be at max cost
        assert_eq!(sut, 999993);
    }

    #[test]
    fn test_calculate_min_cost_for_records() {
        let sut = calculate_cost_for_records(0);
        assert_eq!(sut, MIN_STORE_COST);
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
    async fn can_store_and_retrieve_chunk() {
        let temp_dir = std::env::temp_dir();
        let store_config = NodeRecordStoreConfig {
            storage_dir: temp_dir,
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

        // Create a chunk
        let chunk_data = Bytes::from_static(b"Test chunk data");
        let chunk = Chunk::new(chunk_data.clone());
        let chunk_address = *chunk.address();

        // Create a record from the chunk
        let record = Record {
            key: NetworkAddress::ChunkAddress(chunk_address).to_record_key(),
            value: chunk_data.to_vec(),
            expires: None,
            publisher: None,
        };

        // Store the chunk using put_verified
        assert!(store
            .put_verified(record.clone(), RecordType::Chunk)
            .is_ok());

        // Mark as stored (simulating the CompletedWrite event)
        store.mark_as_stored(record.key.clone(), RecordType::Chunk);

        // Verify the chunk is stored
        let stored_record = store.get(&record.key);
        assert!(stored_record.is_some(), "Chunk should be stored");

        if let Some(stored) = stored_record {
            assert_eq!(
                stored.value, chunk_data,
                "Stored chunk data should match original"
            );

            let stored_address = ChunkAddress::new(XorName::from_content(&stored.value));
            assert_eq!(
                stored_address, chunk_address,
                "Stored chunk address should match original"
            );
        }

        // Clean up
        store.remove(&record.key);
        assert!(
            store.get(&record.key).is_none(),
            "Chunk should be removed after cleanup"
        );
    }

    #[tokio::test]
    async fn can_store_and_retrieve_scratchpad() -> eyre::Result<()> {
        let temp_dir = std::env::temp_dir();
        let store_config = NodeRecordStoreConfig {
            storage_dir: temp_dir,
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

        // Create a scratchpad
        let unencrypted_scratchpad_data = Bytes::from_static(b"Test scratchpad data");
        let owner_sk = SecretKey::random();
        let owner_pk = owner_sk.public_key();

        let mut scratchpad = Scratchpad::new(owner_pk, 0);

        let _next_version =
            scratchpad.update_and_sign(unencrypted_scratchpad_data.clone(), &owner_sk);

        let scratchpad_address = *scratchpad.address();

        // Create a record from the scratchpad
        let record = Record {
            key: NetworkAddress::ScratchpadAddress(scratchpad_address).to_record_key(),
            value: try_serialize_record(&scratchpad, RecordKind::Scratchpad)?.to_vec(),
            expires: None,
            publisher: None,
        };

        // Store the scratchpad using put_verified
        assert!(store
            .put_verified(
                record.clone(),
                RecordType::NonChunk(XorName::from_content(&record.value))
            )
            .is_ok());

        // Mark as stored (simulating the CompletedWrite event)
        store.mark_as_stored(
            record.key.clone(),
            RecordType::NonChunk(XorName::from_content(&record.value)),
        );

        // Verify the scratchpad is stored
        let stored_record = store.get(&record.key);
        assert!(stored_record.is_some(), "Scratchpad should be stored");

        if let Some(stored) = stored_record {
            let scratchpad = try_deserialize_record::<Scratchpad>(&stored)?;

            let stored_address = scratchpad.address();
            assert_eq!(
                stored_address, &scratchpad_address,
                "Stored scratchpad address should match original"
            );

            let decrypted_data = scratchpad.decrypt_data(&owner_sk)?;

            assert_eq!(
                decrypted_data, unencrypted_scratchpad_data,
                "Stored scratchpad data should match original"
            );
        }

        store.remove(&record.key);
        assert!(
            store.get(&record.key).is_none(),
            "Scratchpad should be removed after cleanup"
        );

        Ok(())
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
                &(0..max_records)
                    .map(|_| rand::random::<u8>())
                    .collect::<Bytes>(),
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

    struct PeerStats {
        address: NetworkAddress,
        rewards_addr: RewardsAddress,
        records_stored: AtomicUsize,
        nanos_earned: AtomicU64,
        payments_received: AtomicUsize,
    }

    // takes a long time to run
    #[ignore]
    #[test]
    fn address_distribution_sim() {
        use rayon::prelude::*;

        // as network saturates, we can see that peers all eventually earn similarly
        let num_of_peers = 5_000;
        let num_of_chunks_per_hour = 1_000_000;
        let max_hours = 50;

        //
        let k = K_VALUE.get();

        let replication_group_size = k / 3;

        // Initialize peers with random addresses
        let mut peers: Vec<PeerStats> = (0..num_of_peers)
            .into_par_iter()
            .map(|_| PeerStats {
                address: NetworkAddress::from_peer(PeerId::random()),
                records_stored: AtomicUsize::new(0),
                nanos_earned: AtomicU64::new(0),
                payments_received: AtomicUsize::new(0),
                rewards_addr: dummy_address(),
            })
            .collect();

        let mut hour = 0;
        let mut total_received_payment_count = 0;

        let peers_len = peers.len();

        // Generate a random sorting target address
        let sorting_target_address =
            NetworkAddress::from_chunk_address(ChunkAddress::new(XorName::default()));

        // Sort all peers based on their distance to the sorting target
        peers.par_sort_by(|a, b| {
            sorting_target_address
                .distance(&a.address)
                .cmp(&sorting_target_address.distance(&b.address))
        });

        loop {
            // Parallel processing of chunks
            let _chunk_results: Vec<_> = (0..num_of_chunks_per_hour)
                .into_par_iter()
                .map(|_| {
                    // Generate a random chunk address
                    let name = xor_name::rand::random();
                    let chunk_address = NetworkAddress::from_chunk_address(ChunkAddress::new(name));

                    let chunk_distance_to_sorting = sorting_target_address.distance(&chunk_address);
                    // Binary search to find the insertion point for the chunk
                    let partition_point = peers.partition_point(|peer| {
                        sorting_target_address.distance(&peer.address) < chunk_distance_to_sorting
                    });

                    // Collect close_group_size closest peers
                    let mut close_group = Vec::with_capacity(replication_group_size);
                    let mut left = partition_point;
                    let mut right = partition_point;

                    while close_group.len() < replication_group_size
                        && (left > 0 || right < peers_len)
                    {
                        if left > 0 {
                            left -= 1;
                            close_group.push(left);
                        }
                        if close_group.len() < replication_group_size && right < peers_len {
                            close_group.push(right);
                            right += 1;
                        }
                    }

                    // Truncate to ensure we have exactly close_group_size peers
                    close_group.truncate(replication_group_size);

                    // Find the cheapest payee among the close group
                    let Ok((payee_index, cost)) = pick_cheapest_payee(&peers, &close_group) else {
                        bail!("Failed to find a payee");
                    };

                    for &peer_index in &close_group {
                        let peer = &peers[peer_index];
                        peer.records_stored.fetch_add(1, Ordering::Relaxed);

                        if peer_index == payee_index {
                            peer.nanos_earned.fetch_add(
                                cost.as_atto().try_into().unwrap_or(u64::MAX),
                                Ordering::Relaxed,
                            );
                            peer.payments_received.fetch_add(1, Ordering::Relaxed);
                        }
                    }

                    Ok(())
                })
                .collect();

            // Parallel reduction to calculate statistics
            let (
                received_payment_count,
                empty_earned_nodes,
                min_earned,
                max_earned,
                min_store_cost,
                max_store_cost,
            ) = peers
                .par_iter()
                .map(|peer| {
                    let cost =
                        calculate_cost_for_records(peer.records_stored.load(Ordering::Relaxed));
                    let earned = peer.nanos_earned.load(Ordering::Relaxed);
                    (
                        peer.payments_received.load(Ordering::Relaxed),
                        if earned == 0 { 1 } else { 0 },
                        earned,
                        earned,
                        cost,
                        cost,
                    )
                })
                .reduce(
                    || (0, 0, u64::MAX, 0, u64::MAX, 0),
                    |a, b| {
                        let (
                            a_received_payment_count,
                            a_empty_earned_nodes,
                            a_min_earned,
                            a_max_earned,
                            a_min_store_cost,
                            a_max_store_cost,
                        ) = a;
                        let (
                            b_received_payment_count,
                            b_empty_earned_nodes,
                            b_min_earned,
                            b_max_earned,
                            b_min_store_cost,
                            b_max_store_cost,
                        ) = b;
                        (
                            a_received_payment_count + b_received_payment_count,
                            a_empty_earned_nodes + b_empty_earned_nodes,
                            a_min_earned.min(b_min_earned),
                            a_max_earned.max(b_max_earned),
                            a_min_store_cost.min(b_min_store_cost),
                            a_max_store_cost.max(b_max_store_cost),
                        )
                    },
                );

            total_received_payment_count += num_of_chunks_per_hour;
            assert_eq!(total_received_payment_count, received_payment_count);

            println!("After the completion of hour {hour} with {num_of_chunks_per_hour} chunks put, there are {empty_earned_nodes} nodes which earned nothing");
            println!("\t\t with storecost variation of (min {min_store_cost} - max {max_store_cost}), and earned variation of (min {min_earned} - max {max_earned})");

            hour += 1;

            // Check termination condition
            if hour == max_hours {
                let acceptable_percentage = 0.01; //%

                // Calculate acceptable empty nodes based on % of total nodes
                let acceptable_empty_nodes =
                    (num_of_peers as f64 * acceptable_percentage).ceil() as usize;

                // Assert conditions for termination
                assert!(
                    empty_earned_nodes <= acceptable_empty_nodes,
                    "More than {acceptable_percentage}% of nodes ({acceptable_empty_nodes}) still not earning: {empty_earned_nodes}"
                );
                assert!(
                    (max_store_cost / min_store_cost) < 1000000,
                    "store cost is not 'balanced', expected ratio max/min to be < 1000000, but was {}",
                    max_store_cost / min_store_cost
                );
                assert!(
                    (max_earned / min_earned) < 500000000,
                    "earning distribution is not balanced, expected to be < 500000000, but was {}",
                    max_earned / min_earned
                );
                break;
            }
        }
    }

    fn pick_cheapest_payee(
        peers: &[PeerStats],
        close_group: &[usize],
    ) -> eyre::Result<(usize, AttoTokens)> {
        let mut costs_vec = Vec::with_capacity(close_group.len());
        let mut address_to_index = BTreeMap::new();

        for &i in close_group {
            let peer = &peers[i];
            address_to_index.insert(peer.address.clone(), i);

            let close_records_stored = peer.records_stored.load(Ordering::Relaxed);
            let cost = AttoTokens::from(calculate_cost_for_records(close_records_stored));

            let quote = PaymentQuote {
                content: XorName::default(), // unimportant for cost calc
                cost,
                timestamp: std::time::SystemTime::now(),
                quoting_metrics: QuotingMetrics {
                    close_records_stored: peer.records_stored.load(Ordering::Relaxed),
                    max_records: MAX_RECORDS_COUNT,
                    received_payment_count: 1, // unimportant for cost calc
                    live_time: 0,              // unimportant for cost calc
                },
                bad_nodes: vec![],
                pub_key: bls::SecretKey::random().public_key().to_bytes().to_vec(),
                signature: vec![],
                rewards_address: peer.rewards_addr, // unimportant for cost calc
            };

            costs_vec.push((peer.address.clone(), peer.rewards_addr, quote));
        }

        // sort by address first
        costs_vec.sort_by(|(a_addr, _, _), (b_addr, _, _)| a_addr.cmp(b_addr));

        let Ok((recip_id, _pk, q)) = get_fees_from_store_cost_responses(costs_vec) else {
            bail!("Failed to get fees from store cost responses")
        };

        let Some(index) = address_to_index
            .get(&NetworkAddress::from_peer(recip_id))
            .copied()
        else {
            bail!("Cannot find the index for the cheapest payee");
        };

        Ok((index, q.cost))
    }
}
