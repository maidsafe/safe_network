// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use libp2p::PeerId;
use sn_protocol::NetworkAddress;
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use crate::CLOSE_GROUP_SIZE;

// The entry is kept in the cache for the provided amount of time.
const CACHE_RETAIN_TIME_LIMIT: Duration = Duration::from_secs(20);

// The maximum number of cache entries to keep.
const MAX_CACHE_ENTRIES: usize = 200;

#[derive(Debug, Clone, Default)]
pub(crate) struct CacheGetClosest {
    cache: HashMap<NetworkAddress, (Vec<PeerId>, Instant)>,
}

impl CacheGetClosest {
    /// Inserts the entry into the cache. If the cache is full, we evict the one that has been inserted long ago.
    /// Does not cache if peers <= CLOSE_GROUP_SIZE
    pub(crate) fn insert(&mut self, key: NetworkAddress, peers: &[PeerId]) {
        self.remove_old_keys();
        if peers.len() < CLOSE_GROUP_SIZE {
            return;
        }

        if self.cache.len() >= MAX_CACHE_ENTRIES {
            self.evict_old_entry();
        }
        let _ = self.cache.insert(key, (peers.to_vec(), Instant::now()));
    }

    pub(crate) fn get(&mut self, key: &NetworkAddress) -> Option<&Vec<PeerId>> {
        self.remove_old_keys();
        self.cache.get(key).map(|(peers, _)| peers)
    }

    fn remove_old_keys(&mut self) {
        self.cache
            .retain(|_, (_, instant)| instant.elapsed() <= CACHE_RETAIN_TIME_LIMIT);
    }

    fn evict_old_entry(&mut self) {
        if let Some(key) = self
            .cache
            .iter()
            .max_by_key(|(_, (_, instant))| instant.elapsed())
            .map(|(key, _)| key.clone())
        {
            self.cache.remove(&key);
        }
    }
}
