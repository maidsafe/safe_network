// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::SwarmDriver;
use libp2p::kad::K_VALUE;
use std::time::{Duration, Instant};
use tokio::time::Interval;

/// The interval in which kad.bootstrap is called
pub(crate) const BOOTSTRAP_INTERVAL: Duration = Duration::from_secs(5);

/// Every BOOTSTRAP_CONNECTED_PEERS_STEP connected peer, we step up the BOOTSTRAP_INTERVAL to slow down bootstrapping
/// process
const BOOTSTRAP_CONNECTED_PEERS_STEP: usize = 5;

/// If the previously added peer has been before LAST_PEER_ADDED_TIME_LIMIT, then we should increase the bootstrapping
/// process.
const LAST_PEER_ADDED_TIME_LIMIT: Duration = Duration::from_secs(360);

/// Number of kad buckets we deem desireable
const DESIRABLE_BUCKET_COUNT: usize = 10;

/// Number of peers we want in the avg bucket
const DESIRED_AVG_BUCKET_SIZE: usize = K_VALUE.get() / 3;

impl SwarmDriver {
    pub(crate) async fn run_bootstrap_continuously(
        &mut self,
        current_bootstrap_interval: Duration,
    ) -> Option<Interval> {
        let all_buckets = self.swarm.behaviour_mut().kademlia.kbuckets();
        let mut bucket_stats = vec![];
        // lets see how many peers we know of
        for kbucket in all_buckets {
            let peers_in_bucket = kbucket.num_entries();
            bucket_stats.push(peers_in_bucket)
        }

        let (should_bootstrap, new_interval) = self
            .bootstrap
            .should_we_bootstrap(bucket_stats, current_bootstrap_interval)
            .await;
        if should_bootstrap {
            self.initiate_bootstrap();
        }
        if let Some(new_interval) = &new_interval {
            debug!(
                "The new bootstrap_interval has been updated to {:?}",
                new_interval.period()
            );
        }
        new_interval
    }

    /// Helper to initiate the Kademlia bootstrap process.
    pub(crate) fn initiate_bootstrap(&mut self) {
        match self.swarm.behaviour_mut().kademlia.bootstrap() {
            Ok(query_id) => {
                debug!("Initiated kad bootstrap process with query id {query_id:?}");
                self.bootstrap.initiated();
            }
            Err(err) => {
                error!("Failed to initiate kad bootstrap with error: {err:?}");
            }
        };
    }
}

/// Tracks and helps with the continuous kad::bootstrapping process
pub(crate) struct ContinuousBootstrap {
    is_ongoing: bool,
    initial_bootstrap_done: bool,
    stop_bootstrapping: bool,
    last_peer_added_instant: Instant,
}

impl ContinuousBootstrap {
    pub(crate) fn new() -> Self {
        Self {
            is_ongoing: false,
            initial_bootstrap_done: false,
            last_peer_added_instant: Instant::now(),
            stop_bootstrapping: false,
        }
    }

    /// The Kademlia Bootstrap request has been sent successfully.
    pub(crate) fn initiated(&mut self) {
        self.is_ongoing = true;
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

    /// A previous Kademlia Bootstrap process has been completed. Now a new bootstrap process can start.
    pub(crate) fn completed(&mut self) {
        self.is_ongoing = false;
    }

    /// Set the flag to stop any further re-bootstrapping.
    pub(crate) fn stop_bootstrapping(&mut self) {
        self.stop_bootstrapping = true;
    }

    /// Returns `true` if we should carry out the Kademlia Bootstrap process immediately.
    /// Also optionally returns the new interval to re-bootstrap.
    pub(crate) async fn should_we_bootstrap(
        &mut self,
        bucket_stats: Vec<usize>,
        current_interval: Duration,
    ) -> (bool, Option<Interval>) {
        let peers_in_rt = bucket_stats.iter().sum::<usize>();

        // stop bootstrapping if flag is set
        if self.stop_bootstrapping {
            info!("stop_bootstrapping flag has been set to true. Disabling further bootstrapping");
            let mut new_interval = tokio::time::interval(Duration::from_secs(86400));
            new_interval.tick().await; // the first tick completes immediately
            return (false, Some(new_interval));
        }

        // kad bootstrap process needs at least one peer in the RT be carried out.
        let should_bootstrap = !self.is_ongoing && peers_in_rt >= 1;

        // if we have less than DESIRABLE_BUCKET_COUNT buckets, then we should bootstrap
        if bucket_stats.len() < DESIRABLE_BUCKET_COUNT {
            info!("We have less than {DESIRABLE_BUCKET_COUNT} buckets. Continuing to bootstrap.");
            return (should_bootstrap, None);
        }

        // If we're at a DESIRABLE_BUCKET_COUNT, we can think about slowing down the bootstrapping process
        // if it has been a while (LAST_PEER_ADDED_TIME_LIMIT) since we have added a new peer to our RT, then, slowdown
        // the bootstrapping process.
        // Don't slow down if we haven't even added one peer to our RT.
        if self.last_peer_added_instant.elapsed() > LAST_PEER_ADDED_TIME_LIMIT {
            info!(
                "It has been {LAST_PEER_ADDED_TIME_LIMIT:?} since we last added a peer to RT. Increasing the rate of continuous bootstrapping"
            );

            let mut new_interval = tokio::time::interval(BOOTSTRAP_INTERVAL * 2);
            new_interval.tick().await; // the first tick completes immediately
            return (should_bootstrap, Some(new_interval));
        }

        // We want to aim for a decent average bucket size.
        // we want the average bucket size to be at least K_VALUE / 3
        if (peers_in_rt / bucket_stats.len()) < DESIRED_AVG_BUCKET_SIZE {
            info!(
                "The average bucket is < {DESIRED_AVG_BUCKET_SIZE} peers. Continuing to bootstrap."
            );
            return (should_bootstrap, None);
        }

        // increment bootstrap_interval in steps of BOOTSTRAP_INTERVAL every BOOTSTRAP_CONNECTED_PEERS_STEP
        let step = peers_in_rt / BOOTSTRAP_CONNECTED_PEERS_STEP;
        let step = std::cmp::max(1, step);
        let new_interval = BOOTSTRAP_INTERVAL * step as u32;
        let new_interval = if new_interval > current_interval {
            info!("More peers have been added to our RT!. Slowing down the continuous bootstrapping process");
            let mut interval = tokio::time::interval(new_interval);
            interval.tick().await; // the first tick completes immediately
            Some(interval)
        } else {
            None
        };
        (should_bootstrap, new_interval)
    }
}
