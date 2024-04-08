// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.
#![allow(clippy::mutable_key_type)]

use crate::target_arch::spawn;
use crate::{event::NetworkEvent, target_arch::Instant};
use libp2p::{
    kad::{RecordKey, K_VALUE},
    PeerId,
};
use sn_protocol::{storage::RecordType, NetworkAddress, PrettyPrintRecordKey};
use std::collections::{hash_map::Entry, BTreeSet, HashMap};
use tokio::{sync::mpsc, time::Duration};

// Max parallel fetches that can be undertaken at the same time.
const MAX_PARALLEL_FETCH: usize = K_VALUE.get();

// The duration after which a peer will be considered failed to fetch data from,
// if no response got from that peer.
// Note this will also cover the period that node self write the fetched copy to disk.
// Hence shall give a longer time as allowance.
const FETCH_TIMEOUT: Duration = Duration::from_secs(20);

// The duration after which a pending entry shall be cleared from the `to_be_fetch` list.
// This is to avoid holding too many outdated entries when the fetching speed is slow.
const PENDING_TIMEOUT: Duration = Duration::from_secs(900);

// The time the entry will be considered as `time out` and to be cleared.
type ReplicationTimeout = Instant;

#[derive(Debug)]
pub(crate) struct ReplicationFetcher {
    self_peer_id: PeerId,
    // Pending entries that to be fetched from the target peer.
    to_be_fetched: HashMap<(RecordKey, RecordType, PeerId), ReplicationTimeout>,
    // Avoid fetching same chunk from different nodes AND carry out too many parallel tasks.
    on_going_fetches: HashMap<(RecordKey, RecordType), (PeerId, ReplicationTimeout)>,
    event_sender: mpsc::Sender<NetworkEvent>,
    /// ilog2 bucket distance range that the incoming key shall be fetched
    distance_range: Option<u32>,
}

impl ReplicationFetcher {
    /// Instantiate a new replication fetcher with passed PeerId.
    pub(crate) fn new(self_peer_id: PeerId, event_sender: mpsc::Sender<NetworkEvent>) -> Self {
        Self {
            self_peer_id,
            to_be_fetched: HashMap::new(),
            on_going_fetches: HashMap::new(),
            event_sender,
            distance_range: None,
        }
    }

    /// Set the distance range.
    pub(crate) fn set_replication_distance_range(&mut self, distance_range: u32) {
        self.distance_range = Some(distance_range);
    }

    // Adds the non existing incoming keys from the peer to the fetcher.
    // Returns the next set of keys that has to be fetched from the peer/network.
    //
    // Note: the `incoming_keys` shall already got filter for existence.
    pub(crate) fn add_keys(
        &mut self,
        holder: PeerId,
        mut incoming_keys: Vec<(NetworkAddress, RecordType)>,
        locally_stored_keys: &HashMap<RecordKey, (NetworkAddress, RecordType)>,
    ) -> Vec<(PeerId, RecordKey)> {
        self.remove_stored_keys(locally_stored_keys);

        let mut keys_to_fetch = vec![];
        // For new data, it will be replicated out in a special replication_list of length 1.
        // And we shall `fetch` that copy immediately, if it's not being fetched.
        if incoming_keys.len() == 1 {
            let (record_address, record_type) = incoming_keys[0].clone();

            let new_data_key = (record_address.to_record_key(), record_type);

            if let Entry::Vacant(entry) = self.on_going_fetches.entry(new_data_key.clone()) {
                let (record_key, _record_type) = new_data_key;
                keys_to_fetch.push((holder, record_key));
                let _ = entry.insert((holder, Instant::now() + FETCH_TIMEOUT));
            }

            // To avoid later on un-necessary actions.
            incoming_keys.clear();
        }

        self.to_be_fetched
            .retain(|_, time_out| *time_out > Instant::now());

        let mut out_of_range_keys = vec![];
        let total_incoming_keys = incoming_keys.len();
        // Filter out those out_of_range ones among the imcoming_keys.
        if let Some(ref distance_range) = self.distance_range {
            let self_address = NetworkAddress::from_peer(self.self_peer_id);

            incoming_keys.retain(|(addr, _record_type)| {
                let is_in_range =
                    self_address.distance(addr).ilog2().unwrap_or(0) <= *distance_range;
                if !is_in_range {
                    out_of_range_keys.push(addr.clone());
                }
                is_in_range
            });
        }

        if !out_of_range_keys.is_empty() {
            info!("Among {total_incoming_keys} incoming replications from {holder:?}, found {} out of range", out_of_range_keys.len());
            for addr in out_of_range_keys.iter() {
                trace!("The incoming record_key {addr:?} is out of range, do not fetch it from {holder:?}");
            }
        }

        // add in-range AND non existing keys to the fetcher
        incoming_keys.into_iter().for_each(|(addr, record_type)| {
            let _ = self
                .to_be_fetched
                .entry((addr.to_record_key(), record_type, holder))
                .or_insert(Instant::now() + PENDING_TIMEOUT);
        });

        keys_to_fetch.extend(self.next_keys_to_fetch());

        keys_to_fetch
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
        self.prune_expired_keys_and_slow_nodes();

        info!("Next to fetch....");

        if self.on_going_fetches.len() >= MAX_PARALLEL_FETCH {
            warn!("Replication Fetcher doesn't have free fetch capacity. Currently has {} entries in queue.",
                self.to_be_fetched.len());
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

        for ((key, t, holder), _) in to_be_fetched_sorted {
            // Already carried out expiration pruning above.
            // Hence here only need to check whether is ongoing fetching.
            // Also avoid fetching same record from different nodes.
            if self.on_going_fetches.len() < MAX_PARALLEL_FETCH
                && !self
                    .on_going_fetches
                    .contains_key(&(key.clone(), t.clone()))
            {
                data_to_fetch.push((*holder, key.clone(), t.clone()));
                let _ = self.on_going_fetches.insert(
                    (key.clone(), t.clone()),
                    (*holder, Instant::now() + FETCH_TIMEOUT),
                );
            }

            // break out the loop early if we can do no more now
            if self.on_going_fetches.len() >= MAX_PARALLEL_FETCH {
                break;
            }
        }

        let pretty_keys: Vec<_> = data_to_fetch
            .iter()
            .map(|(holder, key, t)| (*holder, PrettyPrintRecordKey::from(key), t.clone()))
            .collect();

        if !data_to_fetch.is_empty() {
            debug!(
                "Sending out replication request. Fetching {} keys {:?}",
                data_to_fetch.len(),
                pretty_keys
            );
        }

        data_to_fetch
            .iter()
            .map(|(holder, key, t)| {
                let entry_key = (key.clone(), t.clone(), *holder);
                let _ = self.to_be_fetched.remove(&entry_key);
                (*holder, key.clone())
            })
            .collect()
    }

    // Just remove outdated entries in `on_going_fetch`, indicates a failure to fetch from network.
    // The node then considered to be in trouble and:
    //   1, the pending_entries from that node shall be removed from `to_be_fetched` list.
    //   2, firing event up to notify bad_nodes, hence trigger them to be removed from RT.
    fn prune_expired_keys_and_slow_nodes(&mut self) {
        let mut failed_holders = BTreeSet::default();

        self.on_going_fetches.retain(|_, (peer_id, time_out)| {
            if *time_out < Instant::now() {
                failed_holders.insert(*peer_id);
                false
            } else {
                true
            }
        });

        // now to clear any failed nodes from our lists.
        self.to_be_fetched
            .retain(|(_, _, holder), _| !failed_holders.contains(holder));

        // Such failed_hodlers (if any) shall be reported back and be excluded from RT.
        if !failed_holders.is_empty() {
            self.send_event(NetworkEvent::FailedToFetchHolders(failed_holders));
        }
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
        self.on_going_fetches.retain(|(key, t), _| {
            if let Some((_addr, record_type)) = existing_keys.get(key) {
                // check the address only against similar record types
                t != record_type
            } else {
                true
            }
        });
    }

    /// Sends an event after pushing it off thread so as to be non-blocking
    /// this is a wrapper around the `mpsc::Sender::send` call
    fn send_event(&self, event: NetworkEvent) {
        let event_sender = self.event_sender.clone();
        let capacity = event_sender.capacity();

        // push the event off thread so as to be non-blocking
        let _handle = spawn(async move {
            if capacity == 0 {
                warn!(
                    "NetworkEvent channel is full. Await capacity to send: {:?}",
                    event
                );
            }
            if let Err(error) = event_sender.send(event).await {
                error!("ReplicationFetcher failed to send event: {}", error);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{ReplicationFetcher, FETCH_TIMEOUT, MAX_PARALLEL_FETCH};
    use eyre::Result;
    use libp2p::{kad::RecordKey, PeerId};
    use sn_protocol::{storage::RecordType, NetworkAddress};
    use std::{collections::HashMap, time::Duration};
    use tokio::{sync::mpsc, time::sleep};

    #[tokio::test]
    async fn verify_max_parallel_fetches() -> Result<()> {
        //random peer_id
        let peer_id = PeerId::random();
        let (event_sender, _event_receiver) = mpsc::channel(4);
        let mut replication_fetcher = ReplicationFetcher::new(peer_id, event_sender);
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
        let key_1 = NetworkAddress::from_record_key(&RecordKey::from(random_data));
        let random_data: Vec<u8> = (0..50).map(|_| rand::random::<u8>()).collect();
        let key_2 = NetworkAddress::from_record_key(&RecordKey::from(random_data));
        let keys_to_fetch = replication_fetcher.add_keys(
            PeerId::random(),
            vec![(key_1, RecordType::Chunk), (key_2, RecordType::Chunk)],
            &locally_stored_keys,
        );
        assert!(keys_to_fetch.is_empty());

        // List with length of 1 will be considered as `new data` and to be fetched immediately
        let random_data: Vec<u8> = (0..50).map(|_| rand::random::<u8>()).collect();
        let key = NetworkAddress::from_record_key(&RecordKey::from(random_data));
        let keys_to_fetch = replication_fetcher.add_keys(
            PeerId::random(),
            vec![(key, RecordType::Chunk)],
            &locally_stored_keys,
        );
        assert!(!keys_to_fetch.is_empty());

        sleep(FETCH_TIMEOUT + Duration::from_secs(1)).await;

        // all the previous fetches should have failed and fetching next batch...
        let keys_to_fetch = replication_fetcher.next_keys_to_fetch();
        // but as we've marked the previous fetches as failed, that node should be entirely removed from the list
        // leaving us with just _one_ peer left (but with two entries)
        assert_eq!(keys_to_fetch.len(), 2);
        let keys_to_fetch = replication_fetcher.next_keys_to_fetch();
        assert!(keys_to_fetch.is_empty());

        Ok(())
    }

    #[test]
    fn verify_in_range_check() {
        //random peer_id
        let peer_id = PeerId::random();
        let self_address = NetworkAddress::from_peer(peer_id);
        let (event_sender, _event_receiver) = mpsc::channel(4);
        let mut replication_fetcher = ReplicationFetcher::new(peer_id, event_sender);

        // Set distance range
        let distance_target = NetworkAddress::from_peer(PeerId::random());
        let distance_range = self_address.distance(&distance_target).ilog2().unwrap_or(1);
        replication_fetcher.set_replication_distance_range(distance_range);

        let mut incoming_keys = Vec::new();
        let mut in_range_keys = 0;
        (0..100).for_each(|_| {
            let random_data: Vec<u8> = (0..50).map(|_| rand::random::<u8>()).collect();
            let key = NetworkAddress::from_record_key(&RecordKey::from(random_data));

            if key.distance(&self_address).ilog2().unwrap_or(0) <= distance_range {
                in_range_keys += 1;
            }

            incoming_keys.push((key, RecordType::Chunk));
        });

        let keys_to_fetch =
            replication_fetcher.add_keys(PeerId::random(), incoming_keys, &Default::default());
        assert_eq!(
            keys_to_fetch.len(),
            replication_fetcher.on_going_fetches.len()
        );
        assert_eq!(
            in_range_keys,
            keys_to_fetch.len() + replication_fetcher.to_be_fetched.len()
        );
    }
}
