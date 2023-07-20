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

// Max parallel fetches can be undertaken at the same time.
const MAX_PARALLEL_FETCH: usize = 8;

// The duration after which a peer will be considered failed to fetch data from,
// if no response got from that peer.
const FETCH_FAILED_DURATION: Duration = Duration::from_secs(15);

const MAX_FAILED_ATTEMPTS_PER_PEER: u8 = 1;

// Status of the data fetching progress from the holder.
#[derive(PartialEq, Debug)]
pub(crate) enum HolderStatus {
    Pending,
    OnGoing,
}
type FailedAttempts = u8;
type ReplicationReqSentTime = Instant;

#[derive(Default)]
pub(crate) struct ReplicationFetcher {
    to_be_fetched: HashMap<
        RecordKey,
        BTreeMap<PeerId, (ReplicationReqSentTime, HolderStatus, FailedAttempts)>,
    >,
    on_going_fetches: usize,
}

impl ReplicationFetcher {
    // returns the keys to be fetched by cmd flow, if failed couple times, use the get_record
    pub(crate) fn add_keys(
        &mut self,
        peer_id: PeerId,
        incoming_keys: Vec<NetworkAddress>,
        all_locally_stored_keys: &HashSet<RecordKey>,
    ) -> Vec<(RecordKey, Option<PeerId>)> {
        self.remove_held_keys(all_locally_stored_keys);

        // add non exisiting keys
        incoming_keys
            .into_iter()
            .filter_map(|incoming| incoming.as_record_key())
            .filter(|incoming| !all_locally_stored_keys.contains(incoming))
            .for_each(|incoming| self.add_holder(incoming, peer_id));

        self.next_keys_to_fetch()
    }

    pub(crate) fn notify_about_new_put(
        &mut self,
        new_put: RecordKey,
    ) -> Vec<(RecordKey, Option<PeerId>)> {
        if let Some(holders) = self.to_be_fetched.get(&new_put) {
            // attempt should be reduced if it is ongoing
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

    fn next_keys_to_fetch(&mut self) -> Vec<(RecordKey, Option<PeerId>)> {
        // do not fetch any if max parallel fetches
        if self.on_going_fetches >= MAX_PARALLEL_FETCH {
            return vec![];
        }

        let len = MAX_PARALLEL_FETCH - self.on_going_fetches;

        debug!("Records awaiting fetch: {:?}", self.to_be_fetched.len());
        // Randomize the order of keys to fetch
        let mut rng = thread_rng();
        let mut data_to_fetch = self.to_be_fetched.iter_mut().collect::<Vec<_>>();
        data_to_fetch.shuffle(&mut rng); // devskim: ignore DS148264 - this is crypto secure using os rng

        // if failed to fetch, add the failure counter for that peer and change to Pending
        // but if all peers have MAX_FAILED_ATTEMPTS_PER_PEER / or if more than 3 peers have
        // that, then send in for kad.get_record
        //
        let mut keys_to_fetch = HashMap::new();
        let mut to_be_removed = vec![];
        for (key, holders) in data_to_fetch {
            let mut key_added_to_list = false;

            //todo: shuffle holder so that we don't re attempt the same peer continusouly until MAX_FAILED_ATTEMPTS_PER_PEER
            for (peer_id, (replication_req_time, holder_status, failed_attempts)) in
                holders.iter_mut()
            {
                match holder_status {
                    HolderStatus::Pending => {
                        // if we have added this key/if we have all max keys that we can
                        // replicate, continue to the next holder;
                        if key_added_to_list || keys_to_fetch.len() >= len {
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
                        if *replication_req_time + FETCH_FAILED_DURATION > Instant::now() {
                            // allows it to be re-queued
                            *failed_attempts += 1;
                            *holder_status = HolderStatus::Pending;
                            self.on_going_fetches = self.on_going_fetches.saturating_sub(1);
                        }
                    }
                }
            }

            // if 3+ holders have exhauseted MAX_FAILED_ATTEMPTS_PER_PEER, then add it for kad.get_record
            if holders
                .values()
                .filter(|(_, _, failed_attempts)| *failed_attempts > MAX_FAILED_ATTEMPTS_PER_PEER)
                .count()
                >= 3
            {
                to_be_removed.push(key.clone());
                keys_to_fetch.insert(key.clone(), None);
                // network fetch (kad.get_record) is not counted for MAX_PARALLEL_FETCH
                self.on_going_fetches = self.on_going_fetches.saturating_sub(1);
            }
        }

        for failed_key in to_be_removed {
            let _ = self.to_be_fetched.remove(&failed_key);
        }

        trace!("replication fetcher, keys to fetch {keys_to_fetch:?}");

        keys_to_fetch
            .into_iter()
            .map(|(key, peer)| (key, peer))
            .collect::<Vec<_>>()
    }

    /// Remove keys that we hold already and no longer need to be replicated.
    fn remove_held_keys(&mut self, existing_keys: &HashSet<RecordKey>) {
        self.to_be_fetched
            .retain(|key, _| !existing_keys.contains(key));
    }

    fn add_holder(&mut self, key: RecordKey, peer_id: PeerId) {
        let holders = self.to_be_fetched.entry(key).or_insert(Default::default());
        let _ = holders
            .entry(peer_id)
            .or_insert((Instant::now(), HolderStatus::Pending, 0));
    }
}
