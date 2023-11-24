// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use libp2p::{kad::KBucketKey, PeerId};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use sn_protocol::NetworkAddress;
use std::{
    collections::{hash_map::Entry, HashMap, VecDeque},
    time::Instant,
};

const INITIAL_GENERATION_ATTEMPTS: usize = 10_000;
const GENERATION_ATTEMPTS: usize = 1_000;
const MAX_PEERS_PER_BUCKET: usize = 5;

#[derive(Debug, Clone)]
pub(crate) struct NetworkDiscoveryCandidates {
    self_key: KBucketKey<PeerId>,
    candidates: HashMap<u32, VecDeque<NetworkAddress>>,
}

impl NetworkDiscoveryCandidates {
    pub(crate) fn new(self_peer_id: &PeerId) -> Self {
        let start = Instant::now();
        let self_key = KBucketKey::from(*self_peer_id);
        let candidates_vec = Self::generate_candidates(&self_key, INITIAL_GENERATION_ATTEMPTS);

        let mut candidates: HashMap<u32, VecDeque<NetworkAddress>> = HashMap::new();
        for (ilog2, candidate) in candidates_vec {
            match candidates.entry(ilog2) {
                Entry::Occupied(mut entry) => {
                    let entry = entry.get_mut();
                    if entry.len() >= MAX_PEERS_PER_BUCKET {
                        continue;
                    } else {
                        entry.push_back(candidate);
                    }
                }
                Entry::Vacant(entry) => {
                    let _ = entry.insert(VecDeque::from([candidate]));
                }
            }
        }

        info!(
            "Time to generate NetworkDiscoveryCandidates: {:?}",
            start.elapsed()
        );
        let mut buckets_covered = candidates
            .iter()
            .map(|(ilog2, candidates)| (*ilog2, candidates.len()))
            .collect::<Vec<_>>();
        buckets_covered.sort_by_key(|(ilog2, _)| *ilog2);
        info!("The generated network discovery candidates currently cover these ilog2 buckets: {buckets_covered:?}");

        Self {
            self_key,
            candidates,
        }
    }

    pub(crate) fn try_generate_new_candidates(&mut self) {
        let candidates_vec = Self::generate_candidates(&self.self_key, GENERATION_ATTEMPTS);
        for (ilog2, candidate) in candidates_vec {
            match self.candidates.entry(ilog2) {
                Entry::Occupied(mut entry) => {
                    let entry = entry.get_mut();
                    if entry.len() >= MAX_PEERS_PER_BUCKET {
                        // pop the front (as it might have been already used for querying and insert the new one at the back
                        let _ = entry.pop_front();
                        entry.push_back(candidate);
                    } else {
                        entry.push_back(candidate);
                    }
                }
                Entry::Vacant(entry) => {
                    let _ = entry.insert(VecDeque::from([candidate]));
                }
            }
        }
    }

    pub(crate) fn candidates(&self) -> impl Iterator<Item = &NetworkAddress> {
        self.candidates
            .values()
            .filter_map(|candidates| candidates.front())
    }

    fn generate_candidates(
        self_key: &KBucketKey<PeerId>,
        num_to_generate: usize,
    ) -> Vec<(u32, NetworkAddress)> {
        (0..num_to_generate)
            .into_par_iter()
            .filter_map(|_| {
                let candidate = NetworkAddress::from_peer(PeerId::random());
                let candidate_key = candidate.as_kbucket_key();
                let ilog2_distance = candidate_key.distance(&self_key).ilog2()?;
                Some((ilog2_distance, candidate))
            })
            .collect::<Vec<_>>()
    }
}
