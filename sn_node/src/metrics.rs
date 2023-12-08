// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::Marker;
use prometheus_client::{
    encoding::{EncodeLabelSet, EncodeLabelValue},
    metrics::{
        counter::Counter,
        family::Family,
        gauge::Gauge,
        histogram::{exponential_buckets, Histogram},
    },
    registry::Registry,
};

#[derive(Clone)]
pub(crate) struct NodeMetrics {
    /// put record
    put_record_ok: Family<PutRecordOk, Counter>,
    put_record_err: Counter,

    /// replication
    replication_triggered: Counter,
    replication_keys_to_fetch: Histogram,

    // routing table
    peer_added_to_routing_table: Counter,
    peer_removed_from_routing_table: Counter,

    // wallet
    pub(crate) reward_wallet_balance: Gauge,
}

#[derive(EncodeLabelSet, Hash, Clone, Eq, PartialEq, Debug)]
struct PutRecordOk {
    record_type: RecordType,
}

#[derive(EncodeLabelValue, Hash, Clone, Eq, PartialEq, Debug)]
enum RecordType {
    Chunk,
    Register,
    Spend,
}

impl NodeMetrics {
    pub(crate) fn new(registry: &mut Registry) -> Self {
        let sub_registry = registry.sub_registry_with_prefix("sn_node");

        let put_record_ok = Family::default();
        sub_registry.register(
            "put_record_ok",
            "Number of successful record PUTs",
            put_record_ok.clone(),
        );
        let put_record_err = Counter::default();
        sub_registry.register(
            "put_record_err",
            "Number of errors during record PUTs",
            put_record_err.clone(),
        );

        let replication_triggered = Counter::default();
        sub_registry.register(
            "replication_triggered",
            "Number of time that replication has been triggered",
            replication_triggered.clone(),
        );

        // Currently MAX_PARALLEL_FETCH = 2*CLOSE_GROUP_SIZE
        let replication_keys_to_fetch = Histogram::new(exponential_buckets(1.0, 2.0, 4));
        sub_registry.register(
            "replication_keys_to_fetch",
            "Number of replication keys to fetch from the network",
            replication_keys_to_fetch.clone(),
        );

        let peer_added_to_routing_table = Counter::default();
        sub_registry.register(
            "peer_added_to_routing_table",
            "Number of peers that have been added to the Routing Table",
            peer_added_to_routing_table.clone(),
        );

        let peer_removed_from_routing_table = Counter::default();
        sub_registry.register(
            "peer_removed_from_routing_table",
            "Number of peers that have been removed from the Routing Table",
            peer_removed_from_routing_table.clone(),
        );

        let reward_wallet_balance = Gauge::default();
        sub_registry.register(
            "reward_wallet_balance",
            "The number of Nanos in the node reward wallet",
            reward_wallet_balance.clone(),
        );

        Self {
            put_record_ok,
            put_record_err,
            replication_triggered,
            replication_keys_to_fetch,
            peer_added_to_routing_table,
            peer_removed_from_routing_table,
            reward_wallet_balance,
        }
    }

    // Records the metric
    pub(crate) fn record(&self, log_marker: Marker) {
        match log_marker {
            Marker::ValidChunkRecordPutFromNetwork(_) => {
                let _ = self
                    .put_record_ok
                    .get_or_create(&PutRecordOk {
                        record_type: RecordType::Chunk,
                    })
                    .inc();
            }

            Marker::ValidRegisterRecordPutFromNetwork(_) => {
                let _ = self
                    .put_record_ok
                    .get_or_create(&PutRecordOk {
                        record_type: RecordType::Register,
                    })
                    .inc();
            }

            Marker::ValidSpendRecordPutFromNetwork(_) => {
                let _ = self
                    .put_record_ok
                    .get_or_create(&PutRecordOk {
                        record_type: RecordType::Spend,
                    })
                    .inc();
            }

            Marker::RecordRejected(_, _) => {
                let _ = self.put_record_err.inc();
            }

            Marker::ReplicationTriggered => {
                let _ = self.replication_triggered.inc();
            }

            Marker::FetchingKeysForReplication { fetching_keys_len } => self
                .replication_keys_to_fetch
                .observe(fetching_keys_len as f64),

            Marker::PeerAddedToRoutingTable(_) => {
                let _ = self.peer_added_to_routing_table.inc();
            }

            Marker::PeerRemovedFromRoutingTable(_) => {
                let _ = self.peer_removed_from_routing_table.inc();
            }

            _ => {}
        }
    }
}
