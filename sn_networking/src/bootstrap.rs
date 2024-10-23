// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{driver::PendingGetClosestType, SwarmDriver};
use tokio::time::Duration;

use crate::target_arch::Instant;

/// The default interval at which NetworkDiscovery is triggered. The interval is increased as more peers are added to the
/// routing table.
pub(crate) const BOOTSTRAP_INTERVAL: Duration = Duration::from_secs(60);

impl SwarmDriver {
    /// This functions triggers network discovery based on when the last peer was added to the RT and the number of
    /// peers in RT.
    pub(crate) fn run_bootstrap_continuously(&mut self) {
        self.trigger_network_discovery();
    }

    pub(crate) fn trigger_network_discovery(&mut self) {
        let now = Instant::now();
        // Fetches the candidates and also generates new candidates
        for addr in self.network_discovery.candidates() {
            // The query_id is tracked here. This is to update the candidate list of network_discovery with the newly
            // found closest peers. It may fill up the candidate list of closer buckets which are harder to generate.
            let query_id = self
                .swarm
                .behaviour_mut()
                .kademlia
                .get_closest_peers(addr.as_bytes());
            let _ = self.pending_get_closest_peers.insert(
                query_id,
                (
                    addr,
                    PendingGetClosestType::NetworkDiscovery,
                    Default::default(),
                ),
            );
        }

        self.bootstrap.initiated();
        info!("Trigger network discovery took {:?}", now.elapsed());
    }
}

/// Tracks and helps with the continuous kad::bootstrapping process
pub(crate) struct ContinuousBootstrap {
    last_bootstrap_triggered: Option<Instant>,
}

impl ContinuousBootstrap {
    pub(crate) fn new() -> Self {
        Self {
            last_bootstrap_triggered: None,
        }
    }

    /// The Kademlia Bootstrap request has been sent successfully.
    pub(crate) fn initiated(&mut self) {
        self.last_bootstrap_triggered = Some(Instant::now());
    }
}
