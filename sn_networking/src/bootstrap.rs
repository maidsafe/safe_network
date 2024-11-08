// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{driver::PendingGetClosestType, SwarmDriver};
use rand::{rngs::OsRng, Rng};
use tokio::time::Duration;

use crate::target_arch::{interval, Instant, Interval};

/// The default interval at which NetworkDiscovery is triggered. The interval is increased as more peers are added to the
/// routing table.
pub(crate) const BOOTSTRAP_INTERVAL: Duration = Duration::from_secs(10);

/// Every BOOTSTRAP_CONNECTED_PEERS_STEP connected peer, we step up the BOOTSTRAP_INTERVAL to slow down bootstrapping
/// process
const BOOTSTRAP_CONNECTED_PEERS_STEP: u32 = 5;

/// If the previously added peer has been before LAST_PEER_ADDED_TIME_LIMIT, then we should slowdown the bootstrapping
/// process. This is to make sure we don't flood the network with `FindNode` msgs.
const LAST_PEER_ADDED_TIME_LIMIT: Duration = Duration::from_secs(180);

/// A minimum interval to prevent bootstrap got triggered too often
const LAST_BOOTSTRAP_TRIGGERED_TIME_LIMIT: Duration = Duration::from_secs(30);

/// The bootstrap interval to use if we haven't added any new peers in a while.
const NO_PEER_ADDED_SLOWDOWN_INTERVAL_MAX_S: u64 = 600;

impl SwarmDriver {
    /// This functions triggers network discovery based on when the last peer was added to the RT and the number of
    /// peers in RT. The function also returns a new bootstrap interval that is proportional to the number of
    /// peers in RT, so more peers in RT, the longer the interval.
    pub(crate) async fn run_bootstrap_continuously(
        &mut self,
        current_bootstrap_interval: Duration,
    ) -> Option<Interval> {
        let (should_bootstrap, new_interval) = self
            .bootstrap
            .should_we_bootstrap(self.peers_in_rt as u32, current_bootstrap_interval)
            .await;
        if should_bootstrap {
            self.trigger_network_discovery();
        }
        new_interval
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
                (PendingGetClosestType::NetworkDiscovery, Default::default()),
            );
        }

        self.bootstrap.initiated();
        debug!("Trigger network discovery took {:?}", now.elapsed());
    }
}

/// Tracks and helps with the continuous kad::bootstrapping process
pub(crate) struct ContinuousBootstrap {
    initial_bootstrap_done: bool,
    last_peer_added_instant: Instant,
    last_bootstrap_triggered: Option<Instant>,
}

impl ContinuousBootstrap {
    pub(crate) fn new() -> Self {
        Self {
            initial_bootstrap_done: false,
            last_peer_added_instant: Instant::now(),
            last_bootstrap_triggered: None,
        }
    }

    /// The Kademlia Bootstrap request has been sent successfully.
    pub(crate) fn initiated(&mut self) {
        self.last_bootstrap_triggered = Some(Instant::now());
    }

    /// Notify about a newly added peer to the RT. This will help with slowing down the bootstrap process.
    /// Returns `true` if we have to perform the initial bootstrapping.
    pub(crate) fn notify_new_peer(&mut self) -> bool {
        self.last_peer_added_instant = Instant::now();
        // true to kick off the initial bootstrapping. `run_bootstrap_continuously` might kick of so soon that we might
        // not have a single peer in the RT and we'd not perform any bootstrapping for a while.
        if !self.initial_bootstrap_done {
            self.initial_bootstrap_done = true;
            true
        } else {
            false
        }
    }

    /// Returns `true` if we should carry out the Kademlia Bootstrap process immediately.
    /// Also optionally returns the new interval to re-bootstrap.
    #[cfg_attr(target_arch = "wasm32", allow(clippy::unused_async))]
    pub(crate) async fn should_we_bootstrap(
        &self,
        peers_in_rt: u32,
        current_interval: Duration,
    ) -> (bool, Option<Interval>) {
        let is_ongoing = if let Some(last_bootstrap_triggered) = self.last_bootstrap_triggered {
            last_bootstrap_triggered.elapsed() < LAST_BOOTSTRAP_TRIGGERED_TIME_LIMIT
        } else {
            false
        };
        let should_bootstrap = !is_ongoing && peers_in_rt >= 1;

        // if it has been a while (LAST_PEER_ADDED_TIME_LIMIT) since we have added a new peer to our RT, then, slowdown
        // the bootstrapping process.
        // Don't slow down if we haven't even added one peer to our RT.
        if self.last_peer_added_instant.elapsed() > LAST_PEER_ADDED_TIME_LIMIT && peers_in_rt != 0 {
            // To avoid a heart beat like cpu usage due to the 1K candidates generation,
            // randomize the interval within certain range
            let no_peer_added_slowdown_interval: u64 = OsRng.gen_range(
                NO_PEER_ADDED_SLOWDOWN_INTERVAL_MAX_S / 2..NO_PEER_ADDED_SLOWDOWN_INTERVAL_MAX_S,
            );
            let no_peer_added_slowdown_interval_duration =
                Duration::from_secs(no_peer_added_slowdown_interval);
            info!(
                    "It has been {LAST_PEER_ADDED_TIME_LIMIT:?} since we last added a peer to RT. Slowing down the continuous bootstrapping process. Old interval: {current_interval:?}, New interval: {no_peer_added_slowdown_interval_duration:?}"
                );

            // `Interval` ticks immediately for Tokio, but not for `wasmtimer`, which is used for wasm32.
            #[cfg_attr(target_arch = "wasm32", allow(unused_mut))]
            let mut new_interval = interval(no_peer_added_slowdown_interval_duration);
            #[cfg(not(target_arch = "wasm32"))]
            new_interval.tick().await;

            return (should_bootstrap, Some(new_interval));
        }

        // increment bootstrap_interval in steps of BOOTSTRAP_INTERVAL every BOOTSTRAP_CONNECTED_PEERS_STEP
        let step = peers_in_rt / BOOTSTRAP_CONNECTED_PEERS_STEP;
        let step = std::cmp::max(1, step);
        let new_interval = BOOTSTRAP_INTERVAL * step;
        let new_interval = if new_interval > current_interval {
            info!("More peers have been added to our RT!. Slowing down the continuous bootstrapping process. Old interval: {current_interval:?}, New interval: {new_interval:?}");

            // `Interval` ticks immediately for Tokio, but not for `wasmtimer`, which is used for wasm32.
            #[cfg_attr(target_arch = "wasm32", allow(unused_mut))]
            let mut interval = interval(new_interval);
            #[cfg(not(target_arch = "wasm32"))]
            interval.tick().await;

            Some(interval)
        } else {
            None
        };
        (should_bootstrap, new_interval)
    }
}
