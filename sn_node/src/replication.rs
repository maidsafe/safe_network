// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::Node;
use crate::{error::Result, log_markers::Marker};
use libp2p::{kad::kbucket::Distance, kad::KBucketKey, PeerId};
use sn_networking::{sort_peers_by_address, sort_peers_by_key, SwarmCmd, CLOSE_GROUP_SIZE};
use sn_protocol::{
    messages::{Cmd, Query, Request},
    NetworkAddress,
};
use std::collections::BTreeMap;
use tokio::sync::mpsc::Sender;

// To reduce the number of messages exchanged, patch max 500 replication keys into one request.
const MAX_REPLICATION_KEYS_PER_REQUEST: usize = 500;

// Defines how close that a node will trigger replication.
// That is, the node has to be among the REPLICATION_RANGE closest to data,
// to carry out the replication.
const REPLICATION_RANGE: usize = 8;

impl Node {
    pub(crate) async fn update_distance_and_trigger_any_replication(
        &self,
        all_peers: Vec<PeerId>,
        churned_peer: PeerId,
    ) -> Result<()> {
        let our_peer_id = self.network.peer_id;
        let our_address = NetworkAddress::from_peer(our_peer_id);
        let churned_peer_address = NetworkAddress::from_peer(churned_peer);

        // Start updating our distance calcs and trigger any needed replication
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

        let range_were_responsible_for =
            Self::get_data_responsibility_range(&our_address, &sorted_peers)?;
        // Do nothing if self is not among the closest range.
        if our_address.distance(&churned_peer_address) > range_were_responsible_for {
            return Ok(());
        }

        // Update the distance range we are responsible for.
        // this is pushed off thread for SwarmCmd to not block
        self.network
            .set_record_distance_range(range_were_responsible_for)?;

        self.try_trigger_replication_for_sorted_peers(
            &our_peer_id,
            &churned_peer,
            false,
            sorted_peers,
            range_were_responsible_for,
        )
        .await
    }

    /// Get the range of address space that we are responsible for
    /// given a set of sorted peers
    pub(crate) fn get_data_responsibility_range(
        our_address: &NetworkAddress,
        sorted_peers: &[PeerId],
    ) -> Result<Distance> {
        let distance_bar = match sorted_peers.get(CLOSE_GROUP_SIZE) {
            Some(peer) => NetworkAddress::from_peer(*peer).distance(our_address),
            None => {
                debug!("could not obtain distance_bar as sorted_peers.len() <= CLOSE_GROUP_SIZE ");
                return Ok(Distance::default());
            }
        };

        Ok(distance_bar)
    }

    /// Replication is triggered for a given peer and range
    pub(crate) async fn try_trigger_replication_for_sorted_peers(
        &self,
        our_peer_id: &PeerId,
        churned_peer: &PeerId,
        is_dead_peer: bool,
        sorted_peers: Vec<PeerId>,
        data_responsibility_range: Distance,
    ) -> Result<()> {
        let our_address = NetworkAddress::from_peer(*our_peer_id);
        let churned_peer_address = NetworkAddress::from_peer(*churned_peer);

        let swarm_cmd_sender = self.network.get_swarm_cmd_sender();
        // The fetched entries are records that supposed to be held by the churned_peer.
        let entries_to_be_replicated = self
            .network
            .get_record_keys_closest_to_target(&churned_peer_address, data_responsibility_range)
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
        let _ = replications.remove(our_peer_id);
        if is_dead_peer {
            let _ = replications.remove(churned_peer);
        }

        for (peer_id, keys) in replications {
            let (_left, mut remaining_keys) = keys.split_at(0);
            while remaining_keys.len() > MAX_REPLICATION_KEYS_PER_REQUEST {
                let (left, right) = remaining_keys.split_at(MAX_REPLICATION_KEYS_PER_REQUEST);
                remaining_keys = right;
                Self::send_replicate_cmd_without_wait(
                    swarm_cmd_sender.clone(),
                    &our_address,
                    peer_id,
                    left.to_vec(),
                )?;
            }
            Self::send_replicate_cmd_without_wait(
                swarm_cmd_sender.clone(),
                &our_address,
                peer_id,
                remaining_keys.to_vec(),
            )?;
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

        let provided_keys_len = keys.len();
        let keys_to_fetch = self
            .network
            .add_keys_to_replication_fetcher(peer_id, keys)
            .await?;

        if keys_to_fetch.is_empty() {
            return Ok(());
        }

        Marker::FetchingKeysForReplication {
            fetching_keys_len: keys_to_fetch.len(),
            provided_keys_len,
            peer_id,
        }
        .log();

        self.fetch_replication_keys_without_wait(keys_to_fetch)?;
        Ok(())
    }

    /// Utility to send `Query::GetReplicatedData` without awaiting for the `Response` at the call
    /// site
    pub(crate) fn fetch_replication_keys_without_wait(
        &self,
        keys_to_fetch: Vec<(PeerId, NetworkAddress)>,
    ) -> Result<()> {
        for (peer, key) in keys_to_fetch {
            trace!("Fetching replication {key:?} from {peer:?}");
            let request = Request::Query(Query::GetReplicatedData {
                requester: NetworkAddress::from_peer(self.network.peer_id),
                address: key,
            });
            self.network.send_req_ignore_reply(request, peer)?
        }
        Ok(())
    }

    // Utility to send `Cmd::Replicate` without awaiting for the `Response` at the call site.
    fn send_replicate_cmd_without_wait(
        swarm_cmd_sender: Sender<SwarmCmd>,
        our_address: &NetworkAddress,
        peer: PeerId,
        keys: Vec<NetworkAddress>,
    ) -> Result<()> {
        let len = keys.len();
        let req = Request::Cmd(Cmd::Replicate {
            holder: our_address.clone(),
            keys,
        });

        let swarm_cmd = SwarmCmd::SendRequest {
            req,
            peer,
            sender: None,
        };

        trace!("Sending a replication list with {len:?} keys to {peer:?}");
        Self::send_swarm_cmd_using_sender(swarm_cmd_sender, swarm_cmd)?;
        Ok(())
    }

    /// Given a sender, will push the cmd off thread
    fn send_swarm_cmd_using_sender(sender: Sender<SwarmCmd>, cmd: SwarmCmd) -> Result<()> {
        let capacity = sender.capacity();

        if capacity == 0 {
            error!("SwarmCmd channel is full. Dropping SwarmCmd: {:?}", cmd);

            // Lets error out just now.
            // return Err(Error::NoSwarmCmdChannelCapacity);
        }

        // Spawn a task to send the SwarmCmd and keep this fn sync
        let _handle = tokio::spawn(async move {
            if let Err(error) = sender.send(cmd).await {
                error!("Failed to send SwarmCmd: {}", error);
            }
        });

        Ok(())
    }
}
