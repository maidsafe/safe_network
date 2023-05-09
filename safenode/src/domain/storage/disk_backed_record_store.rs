// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::network::CLOSE_GROUP_SIZE;
use libp2p::{
    identity::PeerId,
    kad::{
        kbucket::Key as KBucketKey,
    record::{Key, ProviderRecord, Record},
    store::{Error, RecordStore, Result},
    },
};
use smallvec::SmallVec;
use std::{
    borrow::Cow,
    collections::{hash_map, HashMap, HashSet},
    fs,
    path::PathBuf,
    vec,
};

// The maximum number of providers stored for a key. This value should be equal to our chosen
// CLOSE_GROUP_SIZE (kad::replication_factor).
const MAX_PROVIDERS_PER_KEY: usize = CLOSE_GROUP_SIZE;

/// A `RecordStore` that stores records on disk.
pub(crate) struct DiskBackedRecordStore {
    /// The identity of the peer owning the store.
    local_key: KBucketKey<PeerId>,
    /// The configuration of the store.
    config: DiskBackedRecordStoreConfig,
    /// A set of keys, each corresponding to a data `Record` stored on disk.
    disk_backed_records: HashSet<Key>,
    /// The stored provider records.
    providers: HashMap<Key, SmallVec<[ProviderRecord; MAX_PROVIDERS_PER_KEY]>>,
    /// The set of all provider records for the node identified by `local_key`.
    ///
    /// Must be kept in sync with `providers`.
    provided: HashSet<ProviderRecord>,
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
    /// The maximum number of provider records for which the
    /// local node is the provider.
    pub(crate) max_provided_keys: usize,
}

impl Default for DiskBackedRecordStoreConfig {
    fn default() -> Self {
        Self {
            max_records: 1024,
            max_value_bytes: 65 * 1024,
            max_provided_keys: 1024,
            storage_dir: std::env::temp_dir(),
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
            disk_backed_records: HashSet::default(),
            provided: HashSet::default(),
            providers: HashMap::default(),
        }
    }

    /// Retains the records satisfying a predicate.
    #[allow(dead_code)]
    pub(crate) fn retain<F>(&mut self, predicate: F)
    where
        F: Fn(&Key) -> bool,
    {
        let to_be_removed = self
            .disk_backed_records
            .iter()
            .filter(|k| !predicate(k))
            .cloned()
            .collect::<Vec<_>>();

        to_be_removed.iter().for_each(|key| self.remove(key));
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
}

impl RecordStore for DiskBackedRecordStore {
    type RecordsIter<'a> = vec::IntoIter<Cow<'a, Record>>;
    type ProvidedIter<'a> = vec::IntoIter<Cow<'a, ProviderRecord>>;

    fn get(&self, k: &Key) -> Option<Cow<'_, Record>> {
        // When a client calls GET, the request is forwarded to the nodes until one node returns
        // with the record. Thus a node can be bombarded with GET reqs for random keys. These can be safely
        // ignored if we don't have the record locally.
        if !self.disk_backed_records.contains(k) {
            return None;
        }

        let filename = Self::key_to_hex(k);
        let file_path = self.config.storage_dir.join(filename.clone());

        if !file_path.exists() {
            // we went out of sync with the filesystem
            error!(
                "Data not found for the provided key, filename: {filename} should exist locally"
            );
            return None;
        }

        match fs::read(file_path) {
            Ok(contents) => {
                let record = Record {
                    key: k.clone(),
                    value: contents,
                    publisher: None,
                    expires: None,
                };
                Some(Cow::Owned(record))
            }
            Err(err) => {
                error!("Error reading file: {err:?}");
                None
            }
        }
    }

    fn put(&mut self, r: Record) -> Result<()> {
        if r.value.len() >= self.config.max_value_bytes {
            return Err(Error::ValueTooLarge);
        }

        // todo: the key is not tied to the value it contains, hence the value can be overwritten
        // (incase of dbc double spends etc), hence need to deal with those.
        // Maybe implement a RecordHeader to store the type of data we're storing?
        if self.disk_backed_records.contains(&r.key) {
            return Ok(());
        }

        let num_records = self.disk_backed_records.len();
        if num_records >= self.config.max_records {
            return Err(Error::MaxRecords);
        }

        let file_path = self.config.storage_dir.join(Self::key_to_hex(&r.key));
        match fs::write(file_path, r.value) {
            Ok(_) => {
                let _ = !self.disk_backed_records.insert(r.key);
                Ok(())
            }
            Err(err) => {
                error!("Error writing file: {err:?}");
                Ok(())
            }
        }
    }

    fn remove(&mut self, k: &Key) {
        let file_path = self.config.storage_dir.join(Self::key_to_hex(k));
        match fs::remove_file(file_path) {
            Ok(_) => {
                let _ = self.disk_backed_records.remove(k);
            }
            Err(err) => {
                error!("Error while removing file: {err:?}");
            }
        }
    }

    fn records(&self) -> Self::RecordsIter<'_> {
        let mut records = Vec::new();
        for key in self.disk_backed_records.iter() {
            if let Some(record) = self.get(key) {
                records.push(record);
            }
    }
        records.into_iter()
    }

    fn add_provider(&mut self, record: ProviderRecord) -> Result<()> {
        let num_keys = self.providers.len();

        // Obtain the entry
        let providers = match self.providers.entry(record.key.clone()) {
            e @ hash_map::Entry::Occupied(_) => e,
            e @ hash_map::Entry::Vacant(_) => {
                if self.config.max_provided_keys == num_keys {
                    return Err(Error::MaxProvidedKeys);
                }
                e
            }
        }
        .or_insert_with(Default::default);

        if let Some(i) = providers.iter().position(|p| p.provider == record.provider) {
            // In-place update of an existing provider record.
            providers.as_mut()[i] = record;
        } else {
            // It is a new provider record for that key.
            let local_key = self.local_key.clone();
            let key = KBucketKey::new(record.key.clone());
            let provider = KBucketKey::from(record.provider);
            if let Some(i) = providers.iter().position(|p| {
                let pk = KBucketKey::from(p.provider);
                provider.distance(&key) < pk.distance(&key)
            }) {
                // Insert the new provider.
                if local_key.preimage() == &record.provider {
                    let _ = self.provided.insert(record.clone());
                }
                providers.insert(i, record);
                // Remove the excess provider, if any.
                if providers.len() > MAX_PROVIDERS_PER_KEY {
                    if let Some(p) = providers.pop() {
                        let _ = self.provided.remove(&p);
                    }
                }
            } else if providers.len() < MAX_PROVIDERS_PER_KEY {
                // The distance of the new provider to the key is larger than
                // the distance of any existing provider, but there is still room.
                if local_key.preimage() == &record.provider {
                    let _ = self.provided.insert(record.clone());
                }
                providers.push(record);
            }
        }
        Ok(())
    }

    fn providers(&self, key: &Key) -> Vec<ProviderRecord> {
        self.providers
            .get(key)
            .map_or_else(Vec::new, |ps| ps.clone().into_vec())
    }

    fn provided(&self) -> Self::ProvidedIter<'_> {
        self.provided
            .iter()
            .map(Cow::Borrowed)
            .collect::<Vec<_>>()
            .into_iter()
    }

    fn remove_provider(&mut self, key: &Key, provider: &PeerId) {
        if let hash_map::Entry::Occupied(mut e) = self.providers.entry(key.clone()) {
            let providers = e.get_mut();
            if let Some(i) = providers.iter().position(|p| &p.provider == provider) {
                let p = providers.remove(i);
                let _ = self.provided.remove(&p);
            }
            if providers.is_empty() {
                let _ = e.remove();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use libp2p_core::multihash::{Code, Multihash};
    use quickcheck::*;
    use rand::Rng;

    fn random_multihash() -> Multihash {
        Multihash::wrap(Code::Sha2_256.into(), &rand::thread_rng().gen::<[u8; 32]>()).unwrap()
    }

    fn distance(r: &ProviderRecord) -> kbucket::Distance {
        kbucket::Key::new(r.key.clone()).distance(&kbucket::Key::from(r.provider))
    }

    #[test]
    fn put_get_remove_record() {
        fn prop(r: Record) {
            let mut store = DiskBackedRecordStore::new(PeerId::random());
            assert!(store.put(r.clone()).is_ok());
            assert_eq!(Some(Cow::Borrowed(&r)), store.get(&r.key));
            store.remove(&r.key);
            assert!(store.get(&r.key).is_none());
        }
        quickcheck(prop as fn(_))
    }

    #[test]
    fn add_get_remove_provider() {
        fn prop(r: ProviderRecord) {
            let mut store = DiskBackedRecordStore::new(PeerId::random());
            assert!(store.add_provider(r.clone()).is_ok());
            assert!(store.providers(&r.key).contains(&r));
            store.remove_provider(&r.key, &r.provider);
            assert!(!store.providers(&r.key).contains(&r));
        }
        quickcheck(prop as fn(_))
    }

    #[test]
    fn providers_ordered_by_distance_to_key() {
        fn prop(providers: Vec<kbucket::Key<PeerId>>) -> bool {
            let mut store = DiskBackedRecordStore::new(PeerId::random());
            let key = Key::from(random_multihash());

            let mut records = providers
                .into_iter()
                .map(|p| ProviderRecord::new(key.clone(), p.into_preimage(), Vec::new()))
                .collect::<Vec<_>>();

            for r in &records {
                assert!(store.add_provider(r.clone()).is_ok());
            }

            records.sort_by_key(distance);
            records.truncate(store.config.max_providers_per_key);

            records == store.providers(&key).to_vec()
        }

        quickcheck(prop as fn(_) -> _)
    }

    #[test]
    fn provided() {
        let id = PeerId::random();
        let mut store = DiskBackedRecordStore::new(id);
        let key = random_multihash();
        let rec = ProviderRecord::new(key, id, Vec::new());
        assert!(store.add_provider(rec.clone()).is_ok());
        assert_eq!(
            vec![Cow::Borrowed(&rec)],
            store.provided().collect::<Vec<_>>()
        );
        store.remove_provider(&rec.key, &id);
        assert_eq!(store.provided().count(), 0);
    }

    #[test]
    fn update_provider() {
        let mut store = DiskBackedRecordStore::new(PeerId::random());
        let key = random_multihash();
        let prv = PeerId::random();
        let mut rec = ProviderRecord::new(key, prv, Vec::new());
        assert!(store.add_provider(rec.clone()).is_ok());
        assert_eq!(vec![rec.clone()], store.providers(&rec.key).to_vec());
        rec.expires = Some(Instant::now());
        assert!(store.add_provider(rec.clone()).is_ok());
        assert_eq!(vec![rec.clone()], store.providers(&rec.key).to_vec());
    }

    #[test]
    fn max_provided_keys() {
        let mut store = DiskBackedRecordStore::new(PeerId::random());
        for _ in 0..store.config.max_provided_keys {
            let key = random_multihash();
            let prv = PeerId::random();
            let rec = ProviderRecord::new(key, prv, Vec::new());
            let _ = store.add_provider(rec);
        }
        let key = random_multihash();
        let prv = PeerId::random();
        let rec = ProviderRecord::new(key, prv, Vec::new());
        match store.add_provider(rec) {
            Err(Error::MaxProvidedKeys) => {}
            _ => panic!("Unexpected result"),
        }
    }
}
