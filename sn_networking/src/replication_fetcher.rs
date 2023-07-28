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
const MAX_PARALLEL_FETCH: usize = 8;

// The duration after which a peer will be considered failed to fetch data from,
// if no response got from that peer.
const FETCH_TIMEOUT: Duration = Duration::from_secs(15);

// The maximum number of retries that is performed per peer.
// Else the key is fetched from the Network
const MAX_RETRIES_PER_PEER: u8 = 1;

// If we have failed to fetch the key from <= PEERS_TRIED_BEFORE_NETWORK_FETCH number of peers, then it is sent out
// to be fetched from the network
const PEERS_TRIED_BEFORE_NETWORK_FETCH: u8 = 3;

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

#[derive(Default)]
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
        self.retain_keys(locally_stored_keys);

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
        // do not fetch any if max parallel fetches
        if self.on_going_fetches >= MAX_PARALLEL_FETCH {
            return vec![];
        }
        let len = MAX_PARALLEL_FETCH - self.on_going_fetches;

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

            //todo: shuffle holder so that we don't re attempt the same peer continuously until MAX_FAILED_ATTEMPTS_PER_PEER
            for (peer_id, (replication_req_time, holder_status, failed_attempts)) in
                holders.iter_mut()
            {
                match holder_status {
                    HolderStatus::Pending => {
                        // if we have added this key/if we have all max keys that we can
                        // replicate, continue to the next holder;
                        // todo: or if max failed attempts
                        if key_added_to_list
                            || keys_to_fetch.len() >= len
                            || *failed_attempts >= MAX_RETRIES_PER_PEER
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
    fn retain_keys(&mut self, existing_keys: &HashSet<RecordKey>) {
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
