// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    error::{Error, Result},
    node::Node,
};
use libp2p::{
    kad::{Quorum, Record, RecordKey},
    PeerId,
};
use sn_networking::{GetRecordCfg, Network};
use sn_protocol::{
    messages::{Query, QueryResponse, Request, Response},
    storage::{try_serialize_record, RecordKind, RecordType},
    NetworkAddress, PrettyPrintRecordKey,
};
use tokio::task::spawn;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

impl Node {
    /// Sends _all_ record keys every interval to all peers within the REPLICATE_RANGE.
    pub(crate) fn try_interval_replication(network: Network) {
        network.trigger_interval_replication()
    }

    /// Cleanup unrelevant records if accumulated too many.
    pub(crate) fn trigger_unrelevant_record_cleanup(network: Network) {
        network.trigger_unrelevant_record_cleanup()
    }

    /// Get the Record from a peer or from the network without waiting.
    pub(crate) fn fetch_replication_keys_without_wait(
        &self,
        keys_to_fetch: Vec<(PeerId, RecordKey)>,
    ) -> Result<()> {
        for (holder, key) in keys_to_fetch {
            let node = self.clone();
            let requester = NetworkAddress::from_peer(self.network().peer_id());
            let _handle = spawn(async move {
                let pretty_key = PrettyPrintRecordKey::from(&key).into_owned();
                debug!("Fetching record {pretty_key:?} from node {holder:?}");
                let req = Request::Query(Query::GetReplicatedRecord {
                    requester,
                    key: NetworkAddress::from_record_key(&key),
                });
                let record_opt = if let Ok(resp) = node.network().send_request(req, holder).await {
                    match resp {
                        Response::Query(QueryResponse::GetReplicatedRecord(result)) => match result
                        {
                            Ok((_holder, record_content)) => Some(record_content),
                            Err(err) => {
                                debug!("Failed fetch record {pretty_key:?} from node {holder:?}, with error {err:?}");
                                None
                            }
                        },
                        other => {
                            debug!("Cannot fetch record {pretty_key:?} from node {holder:?}, with response {other:?}");
                            None
                        }
                    }
                } else {
                    None
                };

                let record = if let Some(record_content) = record_opt {
                    Record::new(key, record_content.to_vec())
                } else {
                    debug!(
                        "Can not fetch record {pretty_key:?} from node {holder:?}, fetching from the network"
                    );
                    let get_cfg = GetRecordCfg {
                        get_quorum: Quorum::One,
                        retry_strategy: None,
                        target_record: None,
                        expected_holders: Default::default(),
                        // This is for replication, which doesn't have target_recrod to verify with.
                        // Hence value of the flag actually doesn't matter.
                        is_register: false,
                    };
                    match node
                        .network()
                        .get_record_from_network(key.clone(), &get_cfg)
                        .await
                    {
                        Ok(record) => record,
                        Err(error) => match error {
                            sn_networking::NetworkError::DoubleSpendAttempt(spends) => {
                                debug!("Failed to fetch record {pretty_key:?} from the network, double spend attempt {spends:?}");

                                let bytes = try_serialize_record(&spends, RecordKind::Spend)?;

                                Record {
                                    key,
                                    value: bytes.to_vec(),
                                    publisher: None,
                                    expires: None,
                                }
                            }
                            other_error => return Err(other_error.into()),
                        },
                    }
                };

                debug!(
                    "Got Replication Record {pretty_key:?} from network, validating and storing it"
                );
                if let Err(err) = node.store_replicated_in_record(record).await {
                    error!("During store replication fetched {pretty_key:?}, got error {err:?}");
                } else {
                    debug!("Completed storing Replication Record {pretty_key:?} from network.");
                }
                Ok::<(), Error>(())
            });
        }
        Ok(())
    }

    /// Replicate a fresh record to its close group peers.
    /// This should not be triggered by a record we receive via replicaiton fetch
    pub(crate) fn replicate_valid_fresh_record(
        &self,
        paid_key: RecordKey,
        record_type: RecordType,
    ) {
        let network = self.network().clone();

        let _handle = spawn(async move {
            network
                .replicate_valid_fresh_record(paid_key, record_type)
                .await;
        });
    }
}

pub struct ReplicationManager {
    max_concurrent_replications: usize,
    active_replications: Arc<AtomicUsize>,
}

impl ReplicationManager {
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            max_concurrent_replications: max_concurrent,
            active_replications: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub async fn replicate(&self, record: Record) -> Result<()> {
        // Prevent too many concurrent operations
        if self.active_replications.load(Ordering::SeqCst) >= self.max_concurrent_replications {
            return Err(Error::ResourceExhausted("Too many active replications".into()));
        }
        
        self.active_replications.fetch_add(1, Ordering::SeqCst);
        
        // TODO: Implement actual replication logic here
        // For now we're just tracking concurrency
        
        self.active_replications.fetch_sub(1, Ordering::SeqCst);
        Ok(())
    }
}
