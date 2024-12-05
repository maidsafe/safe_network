// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use serde::{Deserialize, Serialize};

/// Quoting metrics used to generate a quote, or to track peer's status.
#[derive(Clone, Eq, PartialEq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug)]
pub struct QuotingMetrics {
    /// the records stored
    pub close_records_stored: usize,
    /// the max_records configured
    pub max_records: usize,
    /// number of times that got paid
    pub received_payment_count: usize,
    /// the duration that node keeps connected to the network, measured in hours
    pub live_time: u64,
    /// network density from this node's perspective, which is the responsible_range as well
    /// This could be calculated via sampling, or equation calculation.
    pub network_density: Option<[u8; 32]>,
    /// estimated network size
    pub network_size: Option<u64>,
}

impl QuotingMetrics {
    /// construct an empty QuotingMetrics
    pub fn new() -> Self {
        Self {
            close_records_stored: 0,
            max_records: 0,
            received_payment_count: 0,
            live_time: 0,
            network_density: None,
            network_size: None,
        }
    }
}

impl Default for QuotingMetrics {
    fn default() -> Self {
        Self::new()
    }
}
