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
use tokio::task::JoinHandle;

// To reduce the number of messages exchanged, patch max 500 replication keys into one request.
const MAX_REPLICATION_KEYS_PER_REQUEST: usize = 500;

impl Node {
    /// Replication is triggered when is there is a change in our close group
    pub(crate) async fn try_trigger_replication(&self) -> Result<()> {
        Marker::ReplicationTriggered.log();
        let our_close_group = self.network.get_our_close_group().await?;
        let our_peer_id = self.network.peer_id;
        let our_address = NetworkAddress::from_peer(our_peer_id);

        let all_peers = self.network.get_all_local_peers().await?;
        let all_records = self.network.get_all_local_record_addresses().await?;

        // a key is sent to a peer if the peer is considered to be close to that key
        let mut replicate_to: BTreeMap<PeerId, Vec<NetworkAddress>> = Default::default();
        for key in all_records {
            let sorted_based_on_key =
                sort_peers_by_address(all_peers.clone(), &key, CLOSE_GROUP_SIZE)?;

            for peer in our_close_group.iter().filter(|&p| p != &our_peer_id) {
                if sorted_based_on_key.contains(peer) {
                    let keys_to_replicate = replicate_to.entry(*peer).or_insert(Default::default());
                    keys_to_replicate.push(key.clone());
                }
            }
        }

        for (peer_id, keys) in replicate_to {
            let (_left, mut remaining_keys) = keys.split_at(0);
            while remaining_keys.len() > MAX_REPLICATION_KEYS_PER_REQUEST {
                let (left, right) = remaining_keys.split_at(MAX_REPLICATION_KEYS_PER_REQUEST);
                remaining_keys = right;
                self.send_replicate_cmd_without_wait(&our_address, &peer_id, left.to_vec())?;
            }
            self.send_replicate_cmd_without_wait(&our_address, &peer_id, remaining_keys.to_vec())?;
        }

        // set the distance range used for pruning
        let distance_range = match our_close_group.get(CLOSE_GROUP_SIZE) {
            Some(peer) => NetworkAddress::from_peer(*peer).distance(&our_address),
            None => {
                debug!("Could not obtain distance_range");
                return Ok(());
            }
        };
        self.network.set_record_distance_range(distance_range)?;
        Ok(())
    }

    /// Add a list of keys to the Replication fetcher. These keys are later fetched from the peer through the
    /// replication process.
    pub(crate) fn add_keys_to_replication_fetcher(
        &self,
        peer: NetworkAddress,
        keys: Vec<NetworkAddress>,
    ) -> Result<()> {
        let peer_id = if let Some(peer_id) = peer.as_peer_id() {
            peer_id
        } else {
            warn!("Can't parse PeerId from NetworkAddress {peer:?}");
            return Ok(());
        };

        self.network
            .add_keys_to_replication_fetcher(peer_id, keys)?;
        Ok(())
    }

    /// Get the Record from a peer or from the network without waiting.
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
                    let node = self.clone();
                    let _handle: JoinHandle<Result<()>> = tokio::spawn(async move {
                        trace!("Fetching replication {key:?} from the network");
                        let record = node
                            .network
                            .get_record_from_network(key.clone(), None, false)
                            .await?;
                        trace!("Got Replication Record {key:?} from network, validating and storing it");
                        let _ = node.validate_and_store_record(record, false).await?;
                        Ok(())
                    });
                }
            }
        }
        Ok(())
    }

    // Utility to send `Cmd::Replicate` without awaiting for the `Response` at the call site.
    fn send_replicate_cmd_without_wait(
        &self,
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
