// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tracing::info;

/// Tracking used space.
#[derive(Clone, Debug)]
pub(super) struct UsedSpace {
    /// The minimum space that the network requires of the node, to allocate for storage.
    ///
    /// If this value is lower than what is currently used by the other nodes, the node will sooner or later risk getting kicked out
    /// (as it will fill up or fail to serve requested data).
    capacity: usize,
    /// Counts the number of bytes stored to disk.
    used_space: Arc<AtomicUsize>,
}

impl UsedSpace {
    /// Create new `UsedSpace` tracker.
    pub(super) fn new(capacity: usize) -> Self {
        Self {
            capacity,
            used_space: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Increases used space.
    pub(crate) fn increase(&self, size: usize) {
        let _ = self.used_space.fetch_add(size, Ordering::Relaxed);
        let used_space = self.used_space.load(Ordering::Relaxed);
        let used_space_ratio = used_space as f64 / self.capacity as f64;
        info!("Used space: {:?}", used_space);
        info!("Capacity: {:?}", self.capacity);
        info!("Used space ratio: {:?}", used_space_ratio);
    }

    /// Decreases used space.
    pub(crate) fn decrease(&self, size: usize) {
        let _ = self.used_space.fetch_sub(size, Ordering::Relaxed);
    }

    /// This prevents the node to fill up actual disk space
    /// beyond what a node operator deems convenient.
    pub(crate) fn can_add(&self, size: usize) -> bool {
        let current_used_space = self.used_space.load(Ordering::Relaxed);
        current_used_space + size <= self.capacity
    }

    /// Checks if we've reached the minimum expected capacity.
    #[allow(unused)]
    pub(crate) fn has_reached_capacity(&self) -> bool {
        let current_used_space = self.used_space.load(Ordering::Relaxed);
        current_used_space >= self.capacity
    }

    /// Returns the ratio of used space to capacity.
    #[allow(unused)]
    pub(crate) fn ratio(&self) -> f64 {
        let used = self.used_space.load(Ordering::Relaxed);
        used as f64 / self.capacity as f64
    }
}
