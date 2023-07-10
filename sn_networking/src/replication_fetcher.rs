// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use libp2p::PeerId;
use sn_protocol::NetworkAddress;
use std::{
    collections::{BTreeMap, HashSet},
    time::{Duration, Instant},
};

// Max parallel fetches can be undertaken at the same time.
const MAX_PARALLEL_FETCH: usize = 4;

// The duration after which a peer will be considered failed to fetch data from,
// if no response got from that peer.
const FETCH_FAILED_DURATION: Duration = Duration::from_secs(10);

// Status of the data fetching progress from the holder.
#[derive(PartialEq)]
pub(crate) enum HolderStatus {
    Pending,
    OnGoing,
    Failed,
}

#[derive(Default)]
pub(crate) struct ReplicationFetcher {
    to_be_fetched: BTreeMap<NetworkAddress, BTreeMap<PeerId, (Instant, HolderStatus)>>,
    on_going_fetches: usize,
}

impl ReplicationFetcher {
    /// Add a list of keys of a holder.
    /// Return with a list of keys to fetch from holder
    pub(crate) fn add_keys_to_replicate_per_peer(
        &mut self,
        peer_id: PeerId,
        keys: Vec<NetworkAddress>,
    ) -> Vec<(PeerId, NetworkAddress)> {
        for key in keys {
            self.add_holder(key, peer_id);
        }
        self.next_to_fetch()
    }
    /// Remove keys that we hold already and no longer need to be replicated.
    pub(crate) fn remove_held_data(&mut self, keys: &HashSet<NetworkAddress>) {
        self.to_be_fetched.retain(|key, _| !keys.contains(key));
    }

    // Notify the fetch result of a key from a holder.
    // Return with a list of keys to fetch, if presents.
    pub(crate) fn notify_fetch_result(
        &mut self,
        peer_id: PeerId,
        key: NetworkAddress,
        result: bool,
    ) -> Vec<(PeerId, NetworkAddress)> {
        self.on_going_fetches = self.on_going_fetches.saturating_sub(1);

        if result {
            let _ = self.to_be_fetched.remove(&key);
        } else if let Some(holders) = self.to_be_fetched.get_mut(&key) {
            if let Some(status) = holders.get_mut(&peer_id) {
                status.1 = HolderStatus::Failed;
            }
        }
        self.next_to_fetch()
    }

    // Returns a list of keys to fetch
    fn next_to_fetch(&mut self) -> Vec<(PeerId, NetworkAddress)> {
        let mut keys_to_fetch = vec![];
        if self.on_going_fetches >= MAX_PARALLEL_FETCH {
            return keys_to_fetch;
        }

        let len = MAX_PARALLEL_FETCH - self.on_going_fetches;
        let mut all_failed_entries = vec![];
        for (key, holders) in self.to_be_fetched.iter_mut() {
            let mut failed_counter = 0;
            if holders.values().any(|status| {
                HolderStatus::OnGoing == status.1
                    && status.0 + FETCH_FAILED_DURATION > Instant::now()
            }) {
                continue;
            }
            for (peer_id, status) in holders.iter_mut() {
                match status.1 {
                    HolderStatus::Pending => {
                        status.0 = Instant::now();
                        status.1 = HolderStatus::OnGoing;
                        self.on_going_fetches += 1;
                        keys_to_fetch.push((*peer_id, key.clone()));
                        break;
                    }
                    HolderStatus::OnGoing => {
                        trace!("Marking failed {key:?} on {peer_id:?}",);
                        status.1 = HolderStatus::Failed;
                        failed_counter += 1;
                        self.on_going_fetches = self.on_going_fetches.saturating_sub(1);
                    }
                    HolderStatus::Failed => failed_counter += 1,
                }
            }

            if keys_to_fetch.len() >= len {
                break;
            }

            if failed_counter == holders.len() {
                warn!(
                    "Failed to fetch copies of {key:?} from all {:?} holders",
                    holders.len()
                );
                // TODO: fire `get_record` instead?
                all_failed_entries.push(key.clone());
            }
        }

        for failed_key in all_failed_entries {
            let _ = self.to_be_fetched.remove(&failed_key);
        }

        keys_to_fetch
    }

    fn add_holder(&mut self, key: NetworkAddress, peer_id: PeerId) {
        let holders = self.to_be_fetched.entry(key).or_insert(Default::default());
        let _ = holders
            .entry(peer_id)
            .or_insert((Instant::now(), HolderStatus::Pending));
    }
}
