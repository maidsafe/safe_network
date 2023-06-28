// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::error::Result;
use crate::Node;
use libp2p::{
    kad::{KBucketKey, RecordKey},
    PeerId,
};
use sn_networking::{sort_peers_by_address, sort_peers_by_key, CLOSE_GROUP_SIZE};
use sn_protocol::{
    messages::{Cmd, Query, Request},
    storage::ChunkAddress,
    NetworkAddress,
};
use std::collections::{BTreeMap, HashSet};

// To reduce the number of messages exchanged, patch max 500 replication keys into one request.
const MAX_REPLICATION_KEYS_PER_REQUEST: usize = 500;

// Defines how close that a node will trigger replication.
// That is, the node has to be among the REPLICATION_RANGE closest to data,
// to carry out the replication.
const REPLICATION_RANGE: usize = 8;

impl Node {
    /// In case self is not among the closest to the chunk,
    /// replicate the chunk to its closest peers
    pub(crate) async fn try_replicate_an_entry(&mut self, addr: ChunkAddress) {
        let our_address = NetworkAddress::from_peer(self.network.peer_id);
        // The address may need to be put into the `ReplicationList`,
        // hence have to use the form deduced from `RecordKey`.
        let chunk_address = NetworkAddress::from_record_key(RecordKey::new(addr.name()));
        let close_peers =
            if let Ok(peers) = self.network.get_closest_local_peers(&chunk_address).await {
                peers
            } else {
                return;
            };

        // Only carry out replication when self is out of the closest range
        match close_peers.get(CLOSE_GROUP_SIZE - 1) {
            Some(peer) => {
                if our_address.distance(&chunk_address)
                    <= NetworkAddress::from_peer(*peer).distance(&chunk_address)
                {
                    return;
                }
            }
            None => return,
        };
        trace!("Being out of closest range, replicating {addr:?} to {close_peers:?}");
        for peer in close_peers.iter() {
            let _ = self
                .send_replicate_cmd_without_wait(&our_address, peer, vec![chunk_address.clone()])
                .await;
        }
    }

    /// Replication is triggered when the newly added peer or the dead peer was among our closest.
    pub(crate) async fn try_trigger_replication(
        &mut self,
        churned_peer: &PeerId,
        is_dead_peer: bool,
    ) -> Result<()> {
        let our_address = NetworkAddress::from_peer(self.network.peer_id);
        let churned_peer_address = NetworkAddress::from_peer(*churned_peer);

        let all_peers = self.network.get_all_local_peers().await?;
        if all_peers.len() < 2 * CLOSE_GROUP_SIZE {
            return Ok(());
        }

        // Only nearby peers (two times of the CLOSE_GROUP_SIZE) may affect the later on
        // calculation of `closest peers to each entry`.
        // Hence to reduce the computation work, no need to take all peers.
        let sorted_peers: Vec<PeerId> = if let Ok(sorted_peers) =
            sort_peers_by_address(all_peers, &churned_peer_address, 2 * CLOSE_GROUP_SIZE)
        {
            sorted_peers
        } else {
            return Ok(());
        };

        let distance_bar = match sorted_peers.get(CLOSE_GROUP_SIZE) {
            Some(peer) => NetworkAddress::from_peer(*peer).distance(&our_address),
            None => {
                debug!("could not obtain distance_bar as sorted_peers.len() <= CLOSE_GROUP_SIZE ");
                return Ok(());
            }
        };

        // Do nothing if self is not among the closest range.
        if our_address.distance(&churned_peer_address) > distance_bar {
            return Ok(());
        }

        // Setup the record storage distance range.
        self.network.set_record_distance_range(distance_bar).await?;

        // The fetched entries are records that supposed to be held by the churned_peer.
        let entries_to_be_replicated = self
            .network
            .get_record_keys_closest_to_target(&churned_peer_address, distance_bar)
            .await?;

        let mut replications: BTreeMap<PeerId, Vec<NetworkAddress>> = Default::default();
        for key in entries_to_be_replicated.iter() {
            let record_key = KBucketKey::from(key.to_vec());
            let closest_peers: Vec<_> = if let Ok(sorted_peers) =
                sort_peers_by_key(sorted_peers.clone(), &record_key, CLOSE_GROUP_SIZE + 1)
            {
                sorted_peers
            } else {
                continue;
            };

            // Only carry out replication when self within REPLICATION_RANGE
            let replicate_range = match closest_peers.get(REPLICATION_RANGE) {
                Some(peer) => NetworkAddress::from_peer(*peer),
                None => {
                    debug!("could not obtain replicate_range as closest_peers.len() <= REPLICATION_RANGE");
                    continue;
                }
            };

            if our_address.as_kbucket_key().distance(&record_key)
                >= replicate_range.as_kbucket_key().distance(&record_key)
            {
                continue;
            }

            let dsts = if is_dead_peer {
                // To ensure more copies to be retained across the network,
                // make all closest_peers as target in case of peer drop out.
                // This can be reduced depends on the performance.
                closest_peers
            } else {
                vec![*churned_peer]
            };

            for peer in dsts {
                let keys_to_replicate = replications.entry(peer).or_insert(Default::default());
                keys_to_replicate.push(NetworkAddress::from_record_key(key.clone()));
            }
        }

        // Avoid replicate to self or to a dead peer
        let _ = replications.remove(&self.network.peer_id);
        if is_dead_peer {
            let _ = replications.remove(churned_peer);
        }

        for (peer_id, keys) in replications {
            let (_left, mut remaining_keys) = keys.split_at(0);
            while remaining_keys.len() > MAX_REPLICATION_KEYS_PER_REQUEST {
                let (left, right) = remaining_keys.split_at(MAX_REPLICATION_KEYS_PER_REQUEST);
                remaining_keys = right;
                self.send_replicate_cmd_without_wait(&our_address, &peer_id, left.to_vec())
                    .await?;
            }
            self.send_replicate_cmd_without_wait(&our_address, &peer_id, remaining_keys.to_vec())
                .await?;
        }
        Ok(())
    }

    /// Notify a list of keys within a holder to be replicated to self.
    /// The `chunk_storage` is currently held by `swarm_driver` within `network` instance.
    /// Hence has to carry out this notification.
    pub(crate) async fn replication_keys_to_fetch(
        &mut self,
        holder: NetworkAddress,
        keys: Vec<NetworkAddress>,
    ) -> Result<()> {
        let peer_id = if let Some(peer_id) = holder.as_peer_id() {
            peer_id
        } else {
            warn!("Cann't parse PeerId from NetworkAddress {holder:?}");
            return Ok(());
        };
        trace!("Convert {holder:?} to {peer_id:?}");
        let existing_keys: HashSet<NetworkAddress> =
            self.network.get_all_local_record_addresses().await?;
        let non_existing_keys: Vec<NetworkAddress> = keys
            .iter()
            .filter(|key| !existing_keys.contains(key))
            .cloned()
            .collect();

        let keys_to_fetch = self
            .network
            .add_keys_to_replication_fetcher(peer_id, non_existing_keys)
            .await?;
        self.fetch_replication_keys_without_wait(keys_to_fetch)
            .await?;
        Ok(())
    }

    /// Utility to send `Query::GetReplicatedData` without awaiting for the `Response` at the call
    /// site
    pub(crate) async fn fetch_replication_keys_without_wait(
        &mut self,
        keys_to_fetch: Vec<(PeerId, NetworkAddress)>,
    ) -> Result<()> {
        for (peer, key) in keys_to_fetch {
            trace!("Fetching replication {key:?} from {peer:?}");
            let request = Request::Query(Query::GetReplicatedData {
                requester: NetworkAddress::from_peer(self.network.peer_id),
                address: key,
            });
            self.network.send_req_ignore_reply(request, peer).await?
        }
        Ok(())
    }

    // Utility to send `Cmd::Replicate` without awaiting for the `Response` at the call site.
    async fn send_replicate_cmd_without_wait(
        &mut self,
        our_address: &NetworkAddress,
        peer_id: &PeerId,
        keys: Vec<NetworkAddress>,
    ) -> Result<()> {
        let len = keys.len();
        let request = Request::Cmd(Cmd::Replicate {
            holder: our_address.clone(),
            keys,
        });
        self.network
            .send_req_ignore_reply(request, *peer_id)
            .await?;
        trace!("Sending a replication list with {len:?} keys to {peer_id:?}");
        Ok(())
    }
}
