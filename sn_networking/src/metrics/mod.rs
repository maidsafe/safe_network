// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::target_arch::sleep;
use libp2p::metrics::{Metrics as Libp2pMetrics, Recorder};
#[cfg(feature = "upnp")]
use prometheus_client::metrics::{counter::Counter, family::Family};
use prometheus_client::{metrics::gauge::Gauge, registry::Registry};
use sysinfo::{Pid, ProcessRefreshKind, System};
use tokio::time::Duration;

// Implementation to record `libp2p::upnp::Event` metrics
#[cfg(feature = "upnp")]
mod upnp;

const UPDATE_INTERVAL: Duration = Duration::from_secs(15);
const TO_MB: u64 = 1_000_000;

pub(crate) struct NetworkMetrics {
    // Records libp2p related metrics
    // Must directly call self.libp2p_metrics.record(libp2p_event) with Recorder trait in scope. But since we have
    // re-implemented the trait for the wrapper struct, we can instead call self.record(libp2p_event)
    libp2p_metrics: Libp2pMetrics,

    // metrics from sn_networking
    pub(crate) connected_peers: Gauge,
    pub(crate) estimated_network_size: Gauge,
    pub(crate) open_connections: Gauge,
    pub(crate) peers_in_routing_table: Gauge,
    pub(crate) records_stored: Gauge,
    pub(crate) store_cost: Gauge,
    #[cfg(feature = "upnp")]
    pub(crate) upnp_events: Family<upnp::UpnpEventLabels, Counter>,

    // system info
    process_memory_used_mb: Gauge,
    process_cpu_usage_percentage: Gauge,
}

impl NetworkMetrics {
    pub fn new(registry: &mut Registry) -> Self {
        let libp2p_metrics = Libp2pMetrics::new(registry);
        let sub_registry = registry.sub_registry_with_prefix("sn_networking");

        let records_stored = Gauge::default();
        sub_registry.register(
            "records_stored",
            "The number of records stored locally",
            records_stored.clone(),
        );

        let connected_peers = Gauge::default();
        sub_registry.register(
            "connected_peers",
            "The number of peers that we are currently connected to",
            connected_peers.clone(),
        );

        let estimated_network_size = Gauge::default();
        sub_registry.register(
            "estimated_network_size",
            "The estimated number of nodes in the network calculated by the peers in our RT",
            estimated_network_size.clone(),
        );
        let open_connections = Gauge::default();
        sub_registry.register(
            "open_connections",
            "The number of active connections to other peers",
            open_connections.clone(),
        );
        let peers_in_routing_table = Gauge::default();
        sub_registry.register(
            "peers_in_routing_table",
            "The total number of peers in our routing table",
            peers_in_routing_table.clone(),
        );
        let store_cost = Gauge::default();
        sub_registry.register(
            "store_cost",
            "The store cost of the node",
            store_cost.clone(),
        );

        #[cfg(feature = "upnp")]
        let upnp_events = Family::default();
        #[cfg(feature = "upnp")]
        sub_registry.register(
            "upnp_events",
            "Events emitted by the UPnP behaviour",
            upnp_events.clone(),
        );

        let process_memory_used_mb = Gauge::default();
        sub_registry.register(
            "process_memory_used_mb",
            "Memory used by the process in MegaBytes",
            process_memory_used_mb.clone(),
        );

        let process_cpu_usage_percentage = Gauge::default();
        sub_registry.register(
            "process_cpu_usage_percentage",
            "The percentage of CPU used by the process. Value is from 0-100",
            process_cpu_usage_percentage.clone(),
        );

        let network_metrics = Self {
            libp2p_metrics,
            records_stored,
            estimated_network_size,
            connected_peers,
            open_connections,
            peers_in_routing_table,
            store_cost,
            #[cfg(feature = "upnp")]
            upnp_events,
            process_memory_used_mb,
            process_cpu_usage_percentage,
        };

        network_metrics.system_metrics_recorder_task();
        network_metrics
    }

    // Updates registry with sysinfo metrics
    fn system_metrics_recorder_task(&self) {
        // spawn task to record system metrics
        let process_memory_used_mb = self.process_memory_used_mb.clone();
        let process_cpu_usage_percentage = self.process_cpu_usage_percentage.clone();

        let pid = Pid::from_u32(std::process::id());
        let process_refresh_kind = ProcessRefreshKind::everything().without_disk_usage();
        let mut system = System::new_all();
        let physical_core_count = system.physical_core_count();

        tokio::spawn(async move {
            loop {
                system.refresh_process_specifics(pid, process_refresh_kind);
                if let (Some(process), Some(core_count)) =
                    (system.process(pid), physical_core_count)
                {
                    let mem_used = process.memory() / TO_MB;
                    let _ = process_memory_used_mb.set(mem_used as i64);

                    // divide by core_count to get value between 0-100
                    let cpu_usage = process.cpu_usage() / core_count as f32;
                    let _ = process_cpu_usage_percentage.set(cpu_usage as i64);
                }
                sleep(UPDATE_INTERVAL).await;
            }
        });
    }
}

/// Impl the Recorder traits again for our struct.

impl Recorder<libp2p::kad::Event> for NetworkMetrics {
    fn record(&self, event: &libp2p::kad::Event) {
        self.libp2p_metrics.record(event)
    }
}

impl Recorder<libp2p::relay::Event> for NetworkMetrics {
    fn record(&self, event: &libp2p::relay::Event) {
        self.libp2p_metrics.record(event)
    }
}

impl Recorder<libp2p::identify::Event> for NetworkMetrics {
    fn record(&self, event: &libp2p::identify::Event) {
        self.libp2p_metrics.record(event)
    }
}

impl<T> Recorder<libp2p::swarm::SwarmEvent<T>> for NetworkMetrics {
    fn record(&self, event: &libp2p::swarm::SwarmEvent<T>) {
        self.libp2p_metrics.record(event);
    }
}
