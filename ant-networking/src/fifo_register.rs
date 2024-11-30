// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use libp2p::kad::KBucketDistance as Distance;
use std::collections::VecDeque;

pub(crate) struct FifoRegister {
    queue: VecDeque<Distance>,
    max_length: usize,
    #[allow(dead_code)]
    cached_median: Option<Distance>, // Cache for the median result
    is_dirty: bool, // Flag indicating if cache is valid
}

impl FifoRegister {
    // Creates a new FifoRegister with a specified maximum length
    pub(crate) fn new(max_length: usize) -> Self {
        FifoRegister {
            queue: VecDeque::with_capacity(max_length),
            max_length,
            cached_median: None,
            is_dirty: true,
        }
    }

    // Adds an entry to the register, removing excess elements if over max_length
    pub(crate) fn add(&mut self, entry: Distance) {
        if self.queue.len() == self.max_length {
            self.queue.pop_front(); // Remove the oldest element to maintain length
        }
        self.queue.push_back(entry);

        // Mark the cache as invalid since the data has changed
        self.is_dirty = true;
    }

    // Returns the median of the maximum values of the entries
    #[allow(dead_code)]
    pub(crate) fn get_median(&mut self) -> Option<Distance> {
        if self.queue.is_empty() {
            return None; // No median if the queue is empty
        }

        if !self.is_dirty {
            return self.cached_median; // Return cached result if it's valid
        }

        let mut max_values: Vec<Distance> = self.queue.iter().copied().collect();

        max_values.sort_unstable();

        let len = max_values.len();
        // Cache the result and mark the cache as valid
        self.cached_median = Some(max_values[len / 2]);
        self.is_dirty = false;

        self.cached_median
    }
}
