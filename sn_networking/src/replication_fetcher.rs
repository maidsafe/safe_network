// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.
#![allow(clippy::mutable_key_type)]

use libp2p::{kad::RecordKey, PeerId};
use rand::{seq::SliceRandom, thread_rng};
use sn_protocol::NetworkAddress;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    time::{Duration, Instant},
};

// Max parallel fetches that can be undertaken at the same time.
const MAX_PARALLEL_FETCH: usize = 20;

// The duration after which a peer will be considered failed to fetch data from,
// if no response got from that peer.
const FETCH_TIMEOUT: Duration = Duration::from_secs(15);

// The maximum number of retries that is performed per peer.
// Else the key is fetched from the Network
const MAX_RETRIES_PER_PEER: u8 = 1;

// If we have failed to fetch the key from <= PEERS_TRIED_BEFORE_NETWORK_FETCH number of peers, then it is sent out
// to be fetched from the network
const PEERS_TRIED_BEFORE_NETWORK_FETCH: u8 = 5;

// The number of failed attempts while fetching the key from a peer.
type FailedAttempts = u8;
// The time at which the key was sent to be fetched from the peer.
type ReplicationRequestSentTime = Instant;

// Status of the data fetching progress from the holder.
#[derive(PartialEq, Debug)]
pub(crate) enum HolderStatus {
    Pending,
    OnGoing,
}

#[derive(Default, Debug)]
pub(crate) struct ReplicationFetcher {
    to_be_fetched: HashMap<
        RecordKey,
        BTreeMap<PeerId, (ReplicationRequestSentTime, HolderStatus, FailedAttempts)>,
    >,
    on_going_fetches: usize,
}

impl ReplicationFetcher {
    // Adds the non existing incoming keys from the peer to the fetcher. Returns the next set of keys that has to be
    // fetched from the peer/network.
    pub(crate) fn add_keys(
        &mut self,
        peer_id: PeerId,
        incoming_keys: Vec<NetworkAddress>,
        locally_stored_keys: &HashSet<RecordKey>,
    ) -> Vec<(RecordKey, Option<PeerId>)> {
        self.remove_stored_keys(locally_stored_keys);

        // add non existing keys to the fetcher
        incoming_keys
            .into_iter()
            .filter_map(|incoming| incoming.as_record_key())
            .filter(|incoming| !locally_stored_keys.contains(incoming))
            .for_each(|incoming| self.add_holder_pey_key(incoming, peer_id));

        self.next_keys_to_fetch()
    }

    // Notify the replication fetcher about a newly added Record to the node. The corresponding key can now be removed
    // from the replication fetcher.
    // Also returns the next set of keys that has to be fetched from the peer/network.
    pub(crate) fn notify_about_new_put(
        &mut self,
        new_put: RecordKey,
    ) -> Vec<(RecordKey, Option<PeerId>)> {
        // if we're actively fetching for the key, reduce the on_going_fetches before removing the key
        if let Some(holders) = self.to_be_fetched.get(&new_put) {
            if holders
                .values()
                .any(|(_, status, _)| *status == HolderStatus::OnGoing)
            {
                self.on_going_fetches = self.on_going_fetches.saturating_sub(1);
            }
        }
        self.to_be_fetched.remove(&new_put);
        self.next_keys_to_fetch()
    }

    // Returns the set of keys that has to be fetched from the peer/network.
    // If a key has not been sent out to be fetched, it is returned (to be fetched).
    // If a key has not been fetched for `FETCH_TIMEOUT` from a peer, then the failed attempt is increased for that
    // peer and its status is set to be `Pending`. i.e., it can be requeued again.
    // For a key, if we have failed `MAX_RETRIES_PER_PEER` number of times from all holders or
    // `PEERS_TRIED_BEFORE_NETWORK_FETCH` number of holders, then we queue the key to be fetched from the Network.
    // If the key is returned None as the peer, then it has to be fetched from the network.
    fn next_keys_to_fetch(&mut self) -> Vec<(RecordKey, Option<PeerId>)> {
        let no_more_fetches_left = self.on_going_fetches >= MAX_PARALLEL_FETCH;
        let fetches_left = MAX_PARALLEL_FETCH - self.on_going_fetches;

        debug!(
            "Number of records awaiting fetch: {:?}",
            self.to_be_fetched.len()
        );
        // Randomize the order of keys to fetch
        let mut rng = thread_rng();
        let mut data_to_fetch = self.to_be_fetched.iter_mut().collect::<Vec<_>>();
        data_to_fetch.shuffle(&mut rng); // devskim: ignore DS148264 - this is crypto secure using os rng

        let mut keys_to_fetch = HashMap::new();
        let mut to_be_removed = vec![];
        for (key, holders) in data_to_fetch {
            let mut key_added_to_list = false;
            let fetch_for_key_is_ongoing = holders
                .values()
                .find(|(_, holder_status, _)| *holder_status == HolderStatus::OnGoing)
                .map(|(replication_req_time, _, _)| {
                    Instant::now() > *replication_req_time + FETCH_TIMEOUT
                });

            for (peer_id, (replication_req_time, holder_status, failed_attempts)) in
                holders.iter_mut()
            {
                match holder_status {
                    HolderStatus::Pending => {
                        // Check if the fetch for the key is ongoing and the FETCH_TIMEOUT is not hit
                        let fetch_will_end_now = fetch_for_key_is_ongoing.unwrap_or(false);
                        if no_more_fetches_left
                            || key_added_to_list
                            || keys_to_fetch.len() >= fetches_left
                            || *failed_attempts >= MAX_RETRIES_PER_PEER
                            // if fetch is ongoing, but still the FETCH_TIMEOUT is not hit, then
                            // continue. This is to make sure that we queue up that key as soon as
                            // the FETCH_TIMEOUT is hit, else we might have to wait till the next
                            // call to `next_keys_to_fetch` 
                            || (fetch_for_key_is_ongoing.is_some() && !fetch_will_end_now)
                        {
                            continue;
                        }
                        // set status of the holder
                        *replication_req_time = Instant::now();
                        *holder_status = HolderStatus::OnGoing;

                        // add key for replication
                        keys_to_fetch.insert(key.clone(), Some(*peer_id));
                        self.on_going_fetches += 1;
                        key_added_to_list = true;
                    }
                    HolderStatus::OnGoing => {
                        if Instant::now() > *replication_req_time + FETCH_TIMEOUT {
                            warn!("Failed replication fetch! {key:?} from {peer_id:?}");
                            *failed_attempts += 1;
                            // allows it to be re-queued
                            *holder_status = HolderStatus::Pending;
                            self.on_going_fetches = self.on_going_fetches.saturating_sub(1);
                        }
                    }
                }
            }

            // If all holders or PEERS_TRIED_BEFORE_NETWORK_FETCH holders have exhausted MAX_FAILED_ATTEMPTS_PER_PEER,
            // then the key has to be fetched from the network.
            let failed_holders_count = holders
                .values()
                .filter(|(_, _, failed_attempts)| *failed_attempts >= MAX_RETRIES_PER_PEER)
                .count();
            if failed_holders_count == holders.len()
                || failed_holders_count >= PEERS_TRIED_BEFORE_NETWORK_FETCH as usize
            {
                to_be_removed.push(key.clone());
                keys_to_fetch.insert(key.clone(), None);
                // Keys queued for Network fetch are not counted in MAX_PARALLEL_FETCH
                self.on_going_fetches = self.on_going_fetches.saturating_sub(1);
            }
        }

        for failed_key in to_be_removed {
            let _ = self.to_be_fetched.remove(&failed_key);
        }

        trace!("Sending out keys to fetch {keys_to_fetch:?}");

        keys_to_fetch
            .into_iter()
            .map(|(key, peer)| (key, peer))
            .collect::<Vec<_>>()
    }

    /// Remove keys that we hold already and no longer need to be replicated.
    fn remove_stored_keys(&mut self, existing_keys: &HashSet<RecordKey>) {
        self.to_be_fetched
            .retain(|key, _| !existing_keys.contains(key));
    }

    /// Add the holder for the following key
    fn add_holder_pey_key(&mut self, key: RecordKey, peer_id: PeerId) {
        let holders = self.to_be_fetched.entry(key).or_insert(Default::default());
        let _ = holders
            .entry(peer_id)
            .or_insert((Instant::now(), HolderStatus::Pending, 0));
    }
}

#[cfg(test)]
mod tests {
    use super::{ReplicationFetcher, FETCH_TIMEOUT, MAX_PARALLEL_FETCH};
    use eyre::Result;
    use libp2p::{kad::RecordKey, PeerId};
    use sn_protocol::NetworkAddress;
    use std::{collections::HashSet, time::Duration};

    #[tokio::test]
    async fn fetch_from_the_network_if_we_cannot_fetch_from_peer() -> Result<()> {
        let mut replication_fetcher = ReplicationFetcher::default();
        let locally_stored_keys = HashSet::new();

        let random_data: Vec<u8> = (0..50).map(|_| rand::random::<u8>()).collect();
        let key = NetworkAddress::from_record_key(RecordKey::from(random_data));
        let peer = PeerId::random();

        // key should be fetched from peer
        let mut keys_to_fetch =
            replication_fetcher.add_keys(peer, vec![key.clone()], &locally_stored_keys);
        assert_eq!(keys_to_fetch.len(), 1);
        let (fetch_key, fetch_peer) = keys_to_fetch.remove(0);
        assert!(key.as_record_key().is_some_and(|k| k == fetch_key));
        assert!(fetch_peer.is_some_and(|p| p == peer));

        // should not return key as it is being fetched
        let keys_to_fetch = replication_fetcher.next_keys_to_fetch();
        assert_eq!(keys_to_fetch.len(), 0);

        tokio::time::sleep(FETCH_TIMEOUT).await;

        // key should now be fetched from network
        let mut keys_to_fetch =
            replication_fetcher.add_keys(peer, vec![key.clone()], &locally_stored_keys);
        assert_eq!(keys_to_fetch.len(), 1);
        let (fetch_key, fetch_peer) = keys_to_fetch.remove(0);
        assert!(key.as_record_key().is_some_and(|k| k == fetch_key));
        assert!(fetch_peer.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn try_with_multiple_peers_before_fetching_from_network() -> Result<()> {
        let mut replication_fetcher = ReplicationFetcher::default();
        let locally_stored_keys = HashSet::new();
        let mut already_fetched_from = HashSet::new();

        let random_data: Vec<u8> = (0..50).map(|_| rand::random::<u8>()).collect();
        let key = NetworkAddress::from_record_key(RecordKey::from(random_data));
        let peer_1 = PeerId::random();
        let peer_2 = PeerId::random();
        let peer_3 = PeerId::random();
        let peer_4 = PeerId::random();

        // key should be fetched from peer_1
        let mut keys_to_fetch =
            replication_fetcher.add_keys(peer_1, vec![key.clone()], &locally_stored_keys);
        assert_eq!(keys_to_fetch.len(), 1);
        let (fetch_key, fetch_peer) = keys_to_fetch.remove(0);
        assert!(key.as_record_key().is_some_and(|k| k == fetch_key));
        assert!(fetch_peer.is_some_and(|p| p == peer_1));
        already_fetched_from.insert(peer_1);

        // Add peer 2 to 4
        // should not return key as it is being fetched
        let keys_to_fetch =
            replication_fetcher.add_keys(peer_2, vec![key.clone()], &locally_stored_keys);
        assert_eq!(keys_to_fetch.len(), 0);
        let keys_to_fetch =
            replication_fetcher.add_keys(peer_3, vec![key.clone()], &locally_stored_keys);
        assert_eq!(keys_to_fetch.len(), 0);
        let keys_to_fetch =
            replication_fetcher.add_keys(peer_4, vec![key.clone()], &locally_stored_keys);
        assert_eq!(keys_to_fetch.len(), 0);

        tokio::time::sleep(FETCH_TIMEOUT).await;
        // key should be fetched from a random peer that was not already fetched
        let mut keys_to_fetch = replication_fetcher.next_keys_to_fetch();
        let (fetch_key, fetch_peer) = keys_to_fetch.remove(0);
        assert!(key.as_record_key().is_some_and(|k| k == fetch_key));
        assert!(fetch_peer.is_some_and(|p| {
            let res = !already_fetched_from.contains(&p);
            already_fetched_from.insert(p);
            res
        }));

        tokio::time::sleep(FETCH_TIMEOUT).await;
        // key should be fetched from a random peer that was not already fetched
        let mut keys_to_fetch = replication_fetcher.next_keys_to_fetch();
        let (fetch_key, fetch_peer) = keys_to_fetch.remove(0);
        assert!(key.as_record_key().is_some_and(|k| k == fetch_key));
        assert!(fetch_peer.is_some_and(|p| {
            let res = !already_fetched_from.contains(&p);
            already_fetched_from.insert(p);
            res
        }));

        tokio::time::sleep(FETCH_TIMEOUT).await;
        // after PEERS_TRIED_BEFORE_NETWORK_FETCH, we should fetch from network
        let mut keys_to_fetch = replication_fetcher.next_keys_to_fetch();
        let (fetch_key, fetch_peer) = keys_to_fetch.remove(0);
        assert!(key.as_record_key().is_some_and(|k| k == fetch_key));
        assert!(fetch_peer.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn verify_max_parallel_fetches() -> Result<()> {
        let mut replication_fetcher = ReplicationFetcher::default();
        let locally_stored_keys = HashSet::new();

        let peer = PeerId::random();
        let mut incoming_keys = Vec::new();
        (0..MAX_PARALLEL_FETCH).for_each(|_| {
            let random_data: Vec<u8> = (0..50).map(|_| rand::random::<u8>()).collect();
            let key = NetworkAddress::from_record_key(RecordKey::from(random_data));
            incoming_keys.push(key);
        });

        let keys_to_fetch = replication_fetcher.add_keys(peer, incoming_keys, &locally_stored_keys);
        assert_eq!(keys_to_fetch.len(), MAX_PARALLEL_FETCH);

        // we should not fetch anymore keys
        let random_data: Vec<u8> = (0..50).map(|_| rand::random::<u8>()).collect();
        let key = NetworkAddress::from_record_key(RecordKey::from(random_data));
        let keys_to_fetch = replication_fetcher.add_keys(peer, vec![key], &locally_stored_keys);
        assert!(keys_to_fetch.is_empty());

        tokio::time::sleep(FETCH_TIMEOUT + Duration::from_secs(1)).await;

        // all the previous fetches should have failed and should now be fetched from the network
        let keys_to_fetch = replication_fetcher.next_keys_to_fetch();
        assert_eq!(keys_to_fetch.len(), MAX_PARALLEL_FETCH);
        let keys_to_fetch = replication_fetcher.next_keys_to_fetch();
        assert_eq!(keys_to_fetch.len(), 1);

        Ok(())
    }
}
