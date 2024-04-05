// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::Result;

use libp2p::PeerId;
use num::{integer::binomial, pow::Pow};

// Threshold to determine if there is an attack using Kullback-Liebler (KL) divergence
// between model peer ids distribution vs. actual distribution around any point in the address space.
const KL_DIVERGENCE_THRESHOLD: f64 = 10f64; // TODO: find a good value

const K: usize = 20;
const N: usize = 25; // TODO: replace with network size estimation;

pub(super) async fn check_for_sybil_attack(peers: &[PeerId]) -> Result<bool> {
    // TODO: do we go ahead even if we don't have at least K peer ids...?
    info!(
        ">>> CHECKING SYBIL ATTACK WITH {} PEERS: {peers:?}",
        peers.len()
    );
    let q = num_peers_per_cpl(peers)? / K;
    let n = get_net_size_estimate()?;
    let p = compute_model_distribution(n);
    let kl_divergence = compute_kl_divergence(p, q);

    let is_attack = kl_divergence > KL_DIVERGENCE_THRESHOLD;
    Ok(is_attack)
}

// Formula 6 in page 7
fn num_peers_per_cpl(peers: &[PeerId]) -> Result<usize> {
    // TODO!
    Ok(0usize)
}

// Formula 1 and 2 in page ??
fn get_net_size_estimate() -> Result<usize> {
    // TODO!
    Ok(N)
}

// Formula 3 in page 7
fn distrib_j_th_largest_prefix_length(j: usize, x: usize) -> f64 {
    (0..j).fold(0f64, |acc, i| {
        acc + binomial(N, i) as f64
            * (1f64 - 0.5.pow((x + 1) as f64)).pow((N - i) as f64)
            * 0.5.pow(((x + 1) * i) as f64)
    })
}

// Formula 4 in page 7
fn compute_model_distribution(x: usize) -> f64 {
    let model_dist = (1..K + 1).fold(0f64, |acc, j| {
        acc + distrib_j_th_largest_prefix_length(j, x)
            - distrib_j_th_largest_prefix_length(j, x - 1)
    });

    model_dist / K as f64
}

// Formula 5 in page 7
fn compute_kl_divergence(model_dist: f64, peers_per_cpl: usize) -> f64 {
    // TODO!
    model_dist * peers_per_cpl as f64
}
