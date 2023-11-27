// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::node::Node;
use crate::{error::Result, log_markers::Marker};
use libp2p::{
    kad::{Record, RecordKey, K_VALUE},
    PeerId,
};
use sn_networking::{sort_peers_by_address, GetQuorum, CLOSE_GROUP_SIZE};
use sn_protocol::{
    messages::{Cmd, Query, QueryResponse, Request, Response},
    storage::RecordType,
    NetworkAddress, PrettyPrintRecordKey,
};
use std::collections::HashMap;
use tokio::task::{spawn, JoinHandle};

impl Node {
    /// Sends _all_ record keys every interval to all peers.
    pub(crate) async fn try_interval_replication(&self) -> Result<()> {
        let start = std::time::Instant::now();
        trace!("Try trigger interval replication started@{start:?}");
        // Already contains self_peer_id
        let mut closest_k_peers = self.network.get_closest_k_value_local_peers().await?;

        // Do not carry out replication if not many peers present.
        if closest_k_peers.len() < K_VALUE.into() {
            trace!(
                "Not having enough peers to start replication: {:?}/{K_VALUE:?}",
                closest_k_peers.len()
            );
            return Ok(());
        }

        // remove our peer id from the calculations here:
        let _we_were_there = closest_k_peers.remove(&self.network.peer_id);

        // Only grab the closest nodes
        let closest_k_peers = closest_k_peers
            .into_iter()
            .take(CLOSE_GROUP_SIZE + 2)
            .collect::<Vec<_>>();

        trace!("Try trigger interval replication started@{start:?}, peers found_and_sorted, took: {:?}", start.elapsed());
        self.record_metrics(Marker::IntervalReplicationTriggered);

        let our_peer_id = self.network.peer_id;
        let our_address = NetworkAddress::from_peer(our_peer_id);

        #[allow(clippy::mutable_key_type)] // for Bytes in NetworkAddress
        let all_records = self.network.get_all_local_record_addresses().await?;

        if !all_records.is_empty() {
            debug!(
                "Informing all peers of our records. {:?} peers will be informed",
                closest_k_peers.len()
            );
            for peer_id in closest_k_peers {
                self.send_replicate_cmd_without_wait(&our_address, &peer_id, all_records.clone())?;
            }
        }

        info!(
            "Try trigger interval started@{start:?}, took {:?}",
            start.elapsed()
        );
        Ok(())
    }

    /// Get the Record from a peer or from the network without waiting.
    pub(crate) fn fetch_replication_keys_without_wait(
        &self,
        keys_to_fetch: Vec<(PeerId, RecordKey)>,
    ) -> Result<()> {
        for (holder, key) in keys_to_fetch {
            let node = self.clone();
            let requester = NetworkAddress::from_peer(self.network.peer_id);
            let _handle: JoinHandle<Result<()>> = spawn(async move {
                let pretty_key = PrettyPrintRecordKey::from(&key).into_owned();
                trace!("Fetching record {pretty_key:?} from node {holder:?}");
                let req = Request::Query(Query::GetReplicatedRecord {
                    requester,
                    key: NetworkAddress::from_record_key(&key),
                });
                let record_opt = if let Ok(resp) = node.network.send_request(req, holder).await {
                    match resp {
                        Response::Query(QueryResponse::GetReplicatedRecord(result)) => match result
                        {
                            Ok((_holder, record_content)) => Some(record_content),
                            Err(err) => {
                                trace!("Failed fetch record {pretty_key:?} from node {holder:?}, with error {err:?}");
                                None
                            }
                        },
                        other => {
                            trace!("Cannot fetch record {pretty_key:?} from node {holder:?}, with response {other:?}");
                            None
                        }
                    }
                } else {
                    None
                };

                let record = if let Some(record_content) = record_opt {
                    Record::new(key, record_content.to_vec())
                } else {
                    trace!(
                        "Can not fetch record {pretty_key:?} from node {holder:?}, fetching from the network"
                    );
                    node.network
                        .get_record_from_network(
                            key,
                            None,
                            GetQuorum::One,
                            false,
                            Default::default(),
                        )
                        .await?
                };

                trace!(
                    "Got Replication Record {pretty_key:?} from network, validating and storing it"
                );
                let _ = node.store_prepaid_record(record).await?;

                Ok(())
            });
        }
        Ok(())
    }

    /// Replicate a paid record to its close group peers.
    pub(crate) fn replicate_paid_record(&self, paid_key: RecordKey, record_type: RecordType) {
        let network = self.network.clone();

        let _handle = spawn(async move {
            let start = std::time::Instant::now();
            let pretty_key = PrettyPrintRecordKey::from(&paid_key);
            trace!("Start replicate paid record {pretty_key:?} on store");

            // Already contains self_peer_id
            let mut closest_k_peers = match network.get_closest_k_value_local_peers().await {
                Ok(peers) => peers,
                Err(err) => {
                    error!("Replicating paid record {pretty_key:?} get_closest_local_peers errored: {err:?}");
                    return;
                }
            };

            // remove ourself from these calculations
            let _we_were_there = closest_k_peers.remove(&network.peer_id);

            // Do not carry out replication if not many peers present.
            if closest_k_peers.len() < K_VALUE.into() {
                trace!(
                    "Not having enough peers to start replication: {:?}/{K_VALUE:?}",
                    closest_k_peers.len()
                );
                return;
            }

            let data_addr = NetworkAddress::from_record_key(&paid_key);

            let sorted_based_on_addr = match sort_peers_by_address(
                &closest_k_peers,
                &data_addr,
                CLOSE_GROUP_SIZE,
            ) {
                Ok(result) => result,
                Err(err) => {
                    error!(
                            "When replicating paid record {pretty_key:?}, having error when sort {err:?}"
                        );
                    return;
                }
            };

            let our_peer_id = network.peer_id;
            let our_address = NetworkAddress::from_peer(our_peer_id);
            #[allow(clippy::mutable_key_type)] // for Bytes in NetworkAddress
            let keys = HashMap::from([(data_addr.clone(), record_type.clone())]);

            for peer_id in sorted_based_on_addr {
                trace!("Replicating paid record {pretty_key:?} to {peer_id:?}");
                let request = Request::Cmd(Cmd::Replicate {
                    holder: our_address.clone(),
                    keys: keys.clone(),
                });

                let _ = network.send_req_ignore_reply(request, *peer_id);
            }
            trace!(
                "Completed replicate paid record {pretty_key:?} on store, in {:?}",
                start.elapsed()
            );
        });
    }

    // Utility to send `Cmd::Replicate` without awaiting for the `Response` at the call site.
    #[allow(clippy::mutable_key_type)] // for Bytes in NetworkAddress
    fn send_replicate_cmd_without_wait(
        &self,
        our_address: &NetworkAddress,
        peer_id: &PeerId,
        keys: HashMap<NetworkAddress, RecordType>,
    ) -> Result<()> {
        trace!(
            "Sending a replication list of {} keys to {peer_id:?} ",
            keys.len()
        );
        let request = Request::Cmd(Cmd::Replicate {
            holder: our_address.clone(),
            keys,
        });

        self.network.send_req_ignore_reply(request, *peer_id)?;

        Ok(())
    }
}
