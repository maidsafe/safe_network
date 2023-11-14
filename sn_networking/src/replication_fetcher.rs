// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.
#![allow(clippy::mutable_key_type)]

use libp2p::{
    kad::{RecordKey, K_VALUE},
    PeerId,
};
use sn_protocol::{storage::RecordType, NetworkAddress, PrettyPrintRecordKey};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

// Max parallel fetches that can be undertaken at the same time.
const MAX_PARALLEL_FETCH: usize = K_VALUE.get();

// The duration after which a peer will be considered failed to fetch data from,
// if no response got from that peer.
const FETCH_TIMEOUT: Duration = Duration::from_secs(15);

// The time at which the key was sent to be fetched from the peer.
type ReplicationRequestSentTime = Instant;

#[derive(Debug)]
pub(crate) struct ReplicationFetcher {
    self_peer_id: PeerId,
    to_be_fetched: HashMap<(RecordKey, RecordType, PeerId), Option<ReplicationRequestSentTime>>,
    // Avoid fetching same chunk from different nodes AND carry out too many parallel tasks.
    on_going_fetches: HashMap<(RecordKey, RecordType), PeerId>,
}

impl ReplicationFetcher {
    /// Instantiate a new replication fetcher with passed PeerId.
    pub(crate) fn new(self_peer_id: PeerId) -> Self {
        Self {
            self_peer_id,
            to_be_fetched: HashMap::new(),
            on_going_fetches: HashMap::new(),
        }
    }

    // Adds the non existing incoming keys from the peer to the fetcher. Returns the next set of keys that has to be
    // fetched from the peer/network.
    pub(crate) fn add_keys(
        &mut self,
        holder: PeerId,
        incoming_keys: Vec<(NetworkAddress, RecordType)>,
        locally_stored_keys: &HashMap<RecordKey, (NetworkAddress, RecordType)>,
    ) -> Vec<(PeerId, RecordKey)> {
        self.remove_stored_keys(locally_stored_keys);

        // add non existing keys to the fetcher
        incoming_keys
            .into_iter()
            .filter_map(|(addr, record_type)| {
                let key = addr.to_record_key();
                let local = locally_stored_keys.get(&key);

                // if we have a local value of matching record_type, we don't need to fetch it
                if let Some((_, local_record_type)) = local {
                    if local_record_type == &record_type {
                        None
                    } else {
                        Some((key, record_type))
                    }
                } else {
                    Some((key, record_type))
                }
            })
            .for_each(|(key, record_type)| self.add_key(holder, key, record_type));

        self.next_keys_to_fetch()
    }

    // Notify the replication fetcher about a newly added Record to the node.
    // The corresponding key can now be removed from the replication fetcher.
    // Also returns the next set of keys that has to be fetched from the peer/network.
    pub(crate) fn notify_about_new_put(
        &mut self,
        new_put: RecordKey,
        record_type: RecordType,
    ) -> Vec<(PeerId, RecordKey)> {
        self.to_be_fetched
            .retain(|(key, t, _), _| key != &new_put || t != &record_type);

        // if we're actively fetching for the key, reduce the on_going_fetches
        let _ = self.on_going_fetches.remove(&(new_put, record_type));

        self.next_keys_to_fetch()
    }

    // Returns the set of keys that has to be fetched from the peer/network.
    // Target must not be under-fetching
    // and no more than MAX_PARALLEL_FETCH fetches to be undertaken at the same time.
    pub(crate) fn next_keys_to_fetch(&mut self) -> Vec<(PeerId, RecordKey)> {
        self.prune_expired_keys();

        if self.on_going_fetches.len() >= MAX_PARALLEL_FETCH {
            warn!("Replication Fetcher doesn't have free capacity.");
            return vec![];
        }

        if !self.to_be_fetched.is_empty() {
            debug!(
                "Number of records still to be retrieved: {:?}",
                self.to_be_fetched.len()
            );
        }

        let mut data_to_fetch = vec![];
        // Sort to_be_fetched by key closeness to our PeerId
        let mut to_be_fetched_sorted: Vec<_> = self.to_be_fetched.iter_mut().collect();

        let self_address = NetworkAddress::from_peer(self.self_peer_id);

        to_be_fetched_sorted.sort_by(|((a, _, _), _), ((b, _, _), _)| {
            let a = NetworkAddress::from_record_key(a);
            let b = NetworkAddress::from_record_key(b);
            self_address.distance(&a).cmp(&self_address.distance(&b))
        });

        for ((key, t, holder), is_fetching) in to_be_fetched_sorted {
            // Already carried out expiration pruning above.
            // Hence here only need to check whether is ongoing fetching.
            // Also avoid fetching same record from different nodes.
            if is_fetching.is_none()
                && self.on_going_fetches.len() < MAX_PARALLEL_FETCH
                && !self
                    .on_going_fetches
                    .contains_key(&(key.clone(), t.clone()))
            {
                data_to_fetch.push((*holder, key.clone()));
                *is_fetching = Some(Instant::now());
                let _ = self
                    .on_going_fetches
                    .insert((key.clone(), t.clone()), *holder);
            }

            // break out the loop early if we can do no more now
            if self.on_going_fetches.len() >= MAX_PARALLEL_FETCH {
                break;
            }
        }

        let pretty_keys: Vec<_> = data_to_fetch
            .iter()
            .map(|(holder, key)| (*holder, PrettyPrintRecordKey::from(key)))
            .collect();

        if !data_to_fetch.is_empty() {
            debug!(
                "Sending out replication request. Fetching {} keys {:?}",
                data_to_fetch.len(),
                pretty_keys
            );
        }

        data_to_fetch
    }

    // Just remove outdated entries, which indicates a failure to fetch from network.
    // Leave it to to the next round of replication if triggered again.
    fn prune_expired_keys(&mut self) {
        self.to_be_fetched.retain(|(key, t, holder), is_fetching| {
            let is_expired = if let Some(requested_time) = is_fetching {
                if Instant::now() > *requested_time + FETCH_TIMEOUT {
                    self.on_going_fetches.retain(|(data, record_type), node| data != key || node != holder || record_type != t);
                    debug!(
                        "Prune record {:?} at {holder:?} from the replication_fetcher due to timeout.",
                        PrettyPrintRecordKey::from(key)
                    );
                    true
                } else {
                    false
                }
            } else {
                false
            };
            is_fetching.is_none() || !is_expired
        });
    }

    /// Remove keys that we hold already and no longer need to be replicated.
    fn remove_stored_keys(
        &mut self,
        existing_keys: &HashMap<RecordKey, (NetworkAddress, RecordType)>,
    ) {
        self.to_be_fetched.retain(|(key, t, _), _| {
            if let Some((_addr, record_type)) = existing_keys.get(key) {
                // check the address only against similar record types
                t != record_type
            } else {
                true
            }
        });
    }

    /// Add the key if not present yet.
    fn add_key(&mut self, holder: PeerId, key: RecordKey, record_type: RecordType) {
        let _ = self
            .to_be_fetched
            .entry((key, record_type, holder))
            .or_insert(None);
    }
}

#[cfg(test)]
mod tests {
    use super::{ReplicationFetcher, FETCH_TIMEOUT, MAX_PARALLEL_FETCH};
    use eyre::Result;
    use libp2p::{kad::RecordKey, PeerId};
    use sn_protocol::{storage::RecordType, NetworkAddress};
    use std::{collections::HashMap, time::Duration};

    #[tokio::test]
    async fn verify_max_parallel_fetches() -> Result<()> {
        //random peer_id
        let peer_id = PeerId::random();
        let mut replication_fetcher = ReplicationFetcher::new(peer_id);
        let locally_stored_keys = HashMap::new();

        let mut incoming_keys = Vec::new();
        (0..MAX_PARALLEL_FETCH * 2).for_each(|_| {
            let random_data: Vec<u8> = (0..50).map(|_| rand::random::<u8>()).collect();
            let key = NetworkAddress::from_record_key(&RecordKey::from(random_data));
            incoming_keys.push((key, RecordType::Chunk));
        });

        let keys_to_fetch =
            replication_fetcher.add_keys(PeerId::random(), incoming_keys, &locally_stored_keys);
        assert_eq!(keys_to_fetch.len(), MAX_PARALLEL_FETCH);

        // we should not fetch anymore keys
        let random_data: Vec<u8> = (0..50).map(|_| rand::random::<u8>()).collect();
        let key = NetworkAddress::from_record_key(&RecordKey::from(random_data));
        let keys_to_fetch = replication_fetcher.add_keys(
            PeerId::random(),
            vec![(key, RecordType::Chunk)],
            &locally_stored_keys,
        );
        assert!(keys_to_fetch.is_empty());

        tokio::time::sleep(FETCH_TIMEOUT + Duration::from_secs(1)).await;

        // all the previous fetches should have failed and fetching next batch
        let keys_to_fetch = replication_fetcher.next_keys_to_fetch();
        assert_eq!(keys_to_fetch.len(), MAX_PARALLEL_FETCH);
        let keys_to_fetch = replication_fetcher.next_keys_to_fetch();
        assert!(keys_to_fetch.is_empty());

        Ok(())
    }
}
