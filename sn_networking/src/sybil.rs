// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use std::collections::HashMap;

use itertools::Itertools;
use libp2p::{
    kad::{KBucketKey, K_VALUE},
    PeerId,
};
use num::{integer::binomial, pow::Pow};

// Threshold to determine if there is an attack using Kullback-Liebler (KL) divergence
// between model peer ids distribution vs. actual distribution around any point in the address space.
const KL_DIVERGENCE_THRESHOLD: f64 = 10f64; // TODO: find a proper value

const ITERATIONS_FOR_NET_SIZE_ESTIMATION: usize = 500;

// The container maps each random KAD Key to the ordered list
// of its K_VALUE closest peers, sorted by increasing distance. This order
// is a prerequisite for the functions this container is used by,
// i.e. their result is dependant on the correct ordering of these values.
pub(super) type RandomKeysAndClosestPeerIds = Vec<(KBucketKey<Vec<u8>>, Vec<PeerId>)>;

// Given the set of closest K peers ids to the passed content address, return 'true'
// if there is probabilistically a sybil attack around that CID address.
// This implements the algorithm proposed in https://ssg.lancs.ac.uk/wp-content/uploads/ndss_preprint.pdf
pub(super) async fn check_for_sybil_attack(
    peers: &[PeerId],
    cid: KBucketKey<Vec<u8>>,
    random_keys: &RandomKeysAndClosestPeerIds,
) -> bool {
    let k = peers.len();
    info!(">>> CHECKING SYBIL ATTACK WITH {k} PEERS: {peers:?}");

    // FIXME: return error if we don't have at least K peer ids per key
    assert!(k >= K_VALUE.get());
    assert!(random_keys
        .iter()
        .all(|(_, peers)| peers.len() >= K_VALUE.get()));

    let cpls_freqs = num_peers_per_cpl(peers, cid.clone());
    let q = |x| cpls_freqs.get(&x).cloned().unwrap_or(0) as f64 / k as f64;

    let n = get_net_size_estimate(random_keys);
    info!(">>> NET SIZE ESTIMATE: {n}");
    let model_dist = compute_model_distribution(n);
    let p = |x| model_dist.get(&x).cloned().unwrap_or(0f64) / k as f64;

    let kl_divergence = compute_kl_divergence(&p, &q);

    kl_divergence > KL_DIVERGENCE_THRESHOLD
}

// Formula 1 in page 3
// Compute the average distance between each of the passed random keys,
// and their i-th closest peer
fn average_between_keys_and_i_th_closest_peer(
    i: usize,
    random_keys: &RandomKeysAndClosestPeerIds,
) -> f64 {
    let m = random_keys.len() as f64;
    let distances = random_keys.iter().fold(0f64, |acc, (key_j, peers)| {
        let i_th_peer: KBucketKey<PeerId> = peers[i].into();
        let distance = key_j.distance(&i_th_peer).ilog2().unwrap_or(0) as f64;
        acc + distance
    });

    distances / m
}

// Formula 2 in page 3
// Estimates the network size based on the distances between the provided
// random KAD Keys and their closest PeerIds.
fn get_net_size_estimate(random_keys: &RandomKeysAndClosestPeerIds) -> usize {
    let mut best_n_found = 0;
    let mut smallest_value_found = f64::MAX;
    // FIXME: this iteration needs to be smarter
    for n in 0..ITERATIONS_FOR_NET_SIZE_ESTIMATION {
        let value = (0..K_VALUE.get()).fold(0f64, |acc, i| {
            let d_i = average_between_keys_and_i_th_closest_peer(i, random_keys);
            let dist: f64 = d_i - ((2f64.pow(256) * (i + 1) as f64) / (n + 1) as f64);
            acc + dist.pow(2)
        });
        println!(">>> EVAL FOR N {n} gives {value} -- smallest so far {smallest_value_found}");
        if value < smallest_value_found {
            smallest_value_found = value;
            best_n_found = n;
        }
    }

    best_n_found
}

// Formula 3 in page 7
fn distrib_j_th_largest_prefix_length(n: usize, j: usize, x: usize) -> f64 {
    (0..=j).fold(0f64, |acc, i| {
        acc + (binomial(n, i) as f64
            * (1f64 - 0.5.pow((x + 1) as f64)).pow((n - i) as f64)
            * 0.5.pow(((x + 1) * i) as f64))
    })
}

// Formula 4 (partially) in page 7
// Returns a map of common prefix lengths to their probabilistically expected frequency.
fn compute_model_distribution(n: usize) -> HashMap<usize, f64> {
    let f = |x| {
        (0..K_VALUE.get()).fold(0f64, |acc, j| {
            acc + distrib_j_th_largest_prefix_length(n, j, x)
                - distrib_j_th_largest_prefix_length(n, j, x - 1)
        })
    };

    (0..=256).map(|x| (x, f(x))).collect()
}

// Formula 5 in page 7
// Compute the Kullback-Liebler (KL) divergence between the two given distribution
fn compute_kl_divergence(p: &dyn Fn(usize) -> f64, q: &dyn Fn(usize) -> f64) -> f64 {
    (0..256).fold(0f64, |acc, x| {
        let q_x = q(x);
        acc + (q_x * (q_x / p(x)).ln())
    })
}

// Formula 6 (partially) in page 7
// Returns a map with common prefix lengths of given peers and their frequency.
fn num_peers_per_cpl(peers: &[PeerId], cid: KBucketKey<Vec<u8>>) -> HashMap<usize, usize> {
    let cid_bytes = cid.hashed_bytes();
    peers
        .iter()
        .map(|peer| {
            let peer_key: KBucketKey<PeerId> = (*peer).into();
            common_prefix_length(peer_key.hashed_bytes(), cid_bytes)
        })
        .counts()
}

// Helper to calculate number of common prefix bits between two slices
fn common_prefix_length(lhs: &[u8], rhs: &[u8]) -> usize {
    let mut common_prefix_length = 0usize;
    for byte_index in 0..32 {
        if lhs[byte_index] == rhs[byte_index] {
            common_prefix_length += 8;
        } else {
            common_prefix_length += (lhs[byte_index] ^ rhs[byte_index]).leading_zeros() as usize;
            break;
        }
    }
    common_prefix_length
}

#[cfg(test)]
mod tests {
    use crate::sort_peers_by_address;

    use super::{common_prefix_length, get_net_size_estimate, RandomKeysAndClosestPeerIds};

    use libp2p::{kad::K_VALUE, PeerId};
    use sn_protocol::{storage::ChunkAddress, NetworkAddress};

    // we use XorName utilities just because it's easier to build test data with them
    use xor_name::XorName;

    #[test]
    fn test_common_prefix_length() {
        let mut rng = rand::thread_rng();
        let mut lhs = XorName::random(&mut rng);
        assert_eq!(common_prefix_length(&lhs, &lhs), 256);

        let mut rhs = !lhs;
        // let's first make sure lhs != rhs in every bit
        assert_eq!(common_prefix_length(&lhs, &rhs), 0);

        for i in 0..=255 {
            lhs = lhs.with_bit(i, true);
            rhs = rhs.with_bit(i, true);
            assert_eq!(
                i as usize + 1,
                common_prefix_length(&lhs, &rhs),
                "unexpected result from common_prefix_length fn"
            );
        }
    }

    #[test]
    fn test_net_size_estimate() {
        // Build a map with 256 random keys, one for each Kbucket
        // with their corresponding K-closest peers to a random CID;
        // e.g. in Kbucket #2 get the 20 closest peers to a random CID that shares 2 bits as a prefix.
        const NUM_OF_KBUCKETS: usize = 256;
        const NUM_OF_PEERS: usize = 200;
        let random_peers: Vec<PeerId> = (0..NUM_OF_PEERS).map(|_| PeerId::random()).collect();
        let mut random_keys = RandomKeysAndClosestPeerIds::default();

        let mut rng = rand::thread_rng();
        for _i in 0..NUM_OF_KBUCKETS {
            let xorname = XorName::random(&mut rng);
            let address = NetworkAddress::ChunkAddress(ChunkAddress::new(xorname));
            let sorted_peers: Vec<PeerId> =
                sort_peers_by_address(&random_peers, &address, K_VALUE.get())
                    .unwrap()
                    .iter()
                    .map(|p| (**p))
                    .collect();
            random_keys.push((address.as_kbucket_key(), sorted_peers));
        }

        assert_eq!(
            random_keys.iter().filter(|(_, c)| c.len() >= 20).count(),
            NUM_OF_KBUCKETS
        );

        let estimate = get_net_size_estimate(&random_keys);
        println!(">>> NET SIZE ESTIMATE: {estimate}");
        assert_eq!(estimate, NUM_OF_PEERS);
    }
}
