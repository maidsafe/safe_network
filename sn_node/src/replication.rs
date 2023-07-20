// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::Node;
use crate::{error::Result, log_markers::Marker};
use libp2p::kad::RecordKey;
use libp2p::PeerId;
use sn_networking::{sort_peers_by_address, CLOSE_GROUP_SIZE};
use sn_protocol::{
    messages::{Cmd, Query, Request},
    NetworkAddress,
};
use std::collections::BTreeMap;

// To reduce the number of messages exchanged, patch max 500 replication keys into one request.
const MAX_REPLICATION_KEYS_PER_REQUEST: usize = 500;

// Defines how close that a node will trigger replication.
// That is, the node has to be among the REPLICATION_RANGE closest to data,
// to carry out the replication.
// const REPLICATION_RANGE: usize = 8;

impl Node {
    /// Replication is triggered when the newly added peer or the dead peer was among our closest.
    pub(crate) async fn try_trigger_replication(&mut self, new_members: Vec<PeerId>) -> Result<()> {
        Marker::ReplicationTriggered.log();
        debug!("our close group has changed, the new members are {new_members:?}");
        let our_close_group = self.network.get_our_close_group().await?;
        let our_peer_id = self.network.peer_id;
        let our_address = NetworkAddress::from_peer(our_peer_id);

        let all_peers = self.network.get_all_local_peers().await?;

        let entries_to_be_replicated = self.network.get_all_local_record_addresses().await?;
        trace!("entries_to_be_replicated {entries_to_be_replicated:?}");

        let mut replications: BTreeMap<PeerId, Vec<NetworkAddress>> = Default::default();
        for key in entries_to_be_replicated {
            let mut sending_to_n_peers = 0;
            let sorted_based_on_key =
                sort_peers_by_address(all_peers.clone(), &key, CLOSE_GROUP_SIZE + 1)?;

            for peer in our_close_group.iter().filter(|&p| p != &our_peer_id) {
                if sorted_based_on_key.contains(peer) {
                    sending_to_n_peers += 1;
                    let keys_to_replicate = replications.entry(*peer).or_insert(Default::default());
                    keys_to_replicate.push(key.clone());
                }
            }
            trace!("The key is being sent to n_peers {sending_to_n_peers:?} for key {key:?}");
        }

        for (peer_id, keys) in replications {
            let (_left, mut remaining_keys) = keys.split_at(0);
            while remaining_keys.len() > MAX_REPLICATION_KEYS_PER_REQUEST {
                let (left, right) = remaining_keys.split_at(MAX_REPLICATION_KEYS_PER_REQUEST);
                remaining_keys = right;
                self.send_replicate_cmd_without_wait(&our_address, &peer_id, left.to_vec())?;
            }
            self.send_replicate_cmd_without_wait(&our_address, &peer_id, remaining_keys.to_vec())?;
        }
        Ok(())
    }

    /// Notify a list of keys within a holder to be replicated to self.
    /// The `chunk_storage` is currently held by `swarm_driver` within `network` instance.
    /// Hence has to carry out this notification.
    pub(crate) fn replication_keys_to_fetch(
        &mut self,
        holder: NetworkAddress,
        keys: Vec<NetworkAddress>,
    ) -> Result<()> {
        let peer_id = if let Some(peer_id) = holder.as_peer_id() {
            peer_id
        } else {
            warn!("Can't parse PeerId from NetworkAddress {holder:?}");
            return Ok(());
        };
        trace!("Convert {holder:?} to {peer_id:?}");

        self.network
            .add_keys_to_replication_fetcher(peer_id, keys)?;
        Ok(())
    }

    /// Utility to send `Query::GetReplicatedData` without awaiting for the `Response` at the call
    /// site
    pub(crate) fn fetch_replication_keys_without_wait(
        &self,
        keys_to_fetch: Vec<(RecordKey, Option<PeerId>)>,
    ) -> Result<()> {
        for (key, maybe_peer) in keys_to_fetch {
            match maybe_peer {
                Some(peer) => {
                    trace!("Fetching replication {key:?} from {peer:?}");
                    let request = Request::Query(Query::GetReplicatedData {
                        requester: NetworkAddress::from_peer(self.network.peer_id),
                        address: NetworkAddress::from_record_key(key),
                    });
                    self.network.send_req_ignore_reply(request, peer)?
                }
                None => {
                    trace!("Fetching {key:?} from the network, to be implemented");
                }
            }
        }
        Ok(())
    }

    // Utility to send `Cmd::Replicate` without awaiting for the `Response` at the call site.
    fn send_replicate_cmd_without_wait(
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
        self.network.send_req_ignore_reply(request, *peer_id)?;
        trace!("Sending a replication list with {len:?} keys to {peer_id:?}");
        Ok(())
    }
}
