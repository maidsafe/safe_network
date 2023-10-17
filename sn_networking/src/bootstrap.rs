// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::SwarmDriver;
use std::time::Duration;
use tokio::time::Interval;

/// The interval in which kad.bootstrap is called
pub(crate) const BOOTSTRAP_INTERVAL: Duration = Duration::from_secs(5);

/// Every BOOTSTRAP_CONNECTED_PEERS_STEP connected peer, we step up the BOOTSTRAP_INTERVAL to slow down bootstrapping
/// process
const BOOTSTRAP_CONNECTED_PEERS_STEP: u32 = 50;

impl SwarmDriver {
    pub(crate) async fn run_bootstrap_continuously(
        &mut self,
        mut bootstrap_interval: Interval,
    ) -> Interval {
        // kad bootstrap process needs at least one peer in the RT be carried out.
        let connected_peers = self.swarm.connected_peers().count() as u32;
        if !self.bootstrap_ongoing && connected_peers >= 1 {
            debug!(
                "Trying to initiate bootstrap. Current bootstrap_interval {:?}",
                bootstrap_interval.period()
            );
            match self.swarm.behaviour_mut().kademlia.bootstrap() {
                Ok(query_id) => {
                    debug!("Initiated kad bootstrap process with query id {query_id:?}");
                    self.bootstrap_ongoing = true;
                }
                Err(err) => {
                    error!("Failed to initiate kad bootstrap with error: {err:?}")
                }
            };
        }
        // increment bootstrap_interval in steps of INITIAL_BOOTSTRAP_INTERVAL every BOOTSTRAP_CONNECTED_PEERS_STEP
        let step = connected_peers / BOOTSTRAP_CONNECTED_PEERS_STEP;
        let step = std::cmp::max(1, step);
        let new_interval = BOOTSTRAP_INTERVAL * step;
        if new_interval > bootstrap_interval.period() {
            bootstrap_interval = tokio::time::interval(new_interval);
            bootstrap_interval.tick().await; // the first tick completes immediately
        }
        bootstrap_interval
    }
}
