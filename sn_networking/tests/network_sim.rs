// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

#![allow(clippy::mutable_key_type)] // for the Bytes in NetworkAddress

use std::io::Write;

use aes_gcm_siv::aead::KeyInit;
use bls::SecretKey;
use eyre::bail;
use itertools::Itertools;
use libp2p::{
    identity::PeerId,
    kad::{store::RecordStore, K_VALUE},
};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use sn_networking::{MAX_RECORDS_COUNT, MAX_STORE_COST};
use sn_protocol::{storage::ChunkAddress, NetworkAddress};
use sn_transfers::{MainPubkey, NanoTokens, PaymentQuote, QuotingMetrics};
use std::{
    collections::BTreeMap,
    fs,
    sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
    u64, vec,
};
use xor_name::XorName;

use sn_networking::{calculate_cost_for_records, get_fees_from_store_cost_responses};

#[derive(Debug)]
struct PeerStats {
    address: NetworkAddress,
    pk: MainPubkey,
    records_stored: AtomicUsize,
    nanos_earned: AtomicU64,
    payments_received: AtomicUsize,
}

#[test]
fn network_payment_sim() -> eyre::Result<()> {
    use rayon::prelude::*;

    // Create a directory for this run
    let run_dir: std::path::PathBuf = create_run_directory()?;
    let graphs_dir = run_dir.join("graphs");
    let peer_data_dir = run_dir.join("peer_data");

    // Define the matrix of parameters to test
    let peer_counts = vec![50_000, 200_000, 2_000_000];
    let max_payments = vec![100_000, 500_000, 5_000_000];
    let replication_group_sizes = vec![3, 5, 7];

    fs::create_dir_all(&graphs_dir)?;
    fs::create_dir_all(&peer_data_dir)?;

    for &num_of_peers in &peer_counts {
        for &max_payments_attempts_made in &max_payments {
            for &replication_group_size in &replication_group_sizes {
                println!(
                    "Running simulation with {} peers, {} max payments, and replication group size {}",
                    num_of_peers, max_payments_attempts_made, replication_group_size
                );

                // as network saturates, we can see that peers all eventually earn similarly
                let num_of_chunks_per_hour = 400;

                let mut hour = 0;
                let k = K_VALUE.get();

                let failed_payee_finding = AtomicBool::new(false);

                // Initialize peers with random addresses
                let mut peers: Vec<PeerStats> = (0..num_of_peers)
                    .into_par_iter()
                    .map(|_| PeerStats {
                        address: NetworkAddress::from_peer(PeerId::random()),
                        records_stored: AtomicUsize::new(0),
                        nanos_earned: AtomicU64::new(0),
                        payments_received: AtomicUsize::new(0),
                        pk: MainPubkey::new(SecretKey::random().public_key()),
                    })
                    .collect();

                let total_failed_payments = AtomicUsize::new(0);

                let peers_len = peers.len();

                // Generate a random sorting target address
                let sorting_target_address =
                    NetworkAddress::from_chunk_address(ChunkAddress::new(XorName::default()));

                // Sort all peers based on their distance to the sorting target
                peers.par_sort_by(|a, b| {
                    sorting_target_address
                        .distance(&a.address)
                        .cmp(&sorting_target_address.distance(&b.address))
                });

                loop {
                    // Parallel processing of chunks
                    let _chunk_results: Vec<_> = (0..num_of_chunks_per_hour)
                        .into_par_iter()
                        .map(|_| {
                            // Generate a random chunk address
                            let name = xor_name::rand::random();
                            let chunk_address =
                                NetworkAddress::from_chunk_address(ChunkAddress::new(name));

                            let chunk_distance_to_sorting =
                                sorting_target_address.distance(&chunk_address);
                            // Binary search to find the insertion point for the chunk
                            let partition_point = peers.partition_point(|peer| {
                                sorting_target_address.distance(&peer.address)
                                    < chunk_distance_to_sorting
                            });

                            // Collect close_group_size closest peers
                            let mut close_group = Vec::with_capacity(replication_group_size);
                            let mut left = partition_point;
                            let mut right = partition_point;

                            // search extended to find only nodes with less than MAX_RECORDS_COUNT
                            while close_group.len() < replication_group_size
                                && (left > 0 || right < peers_len)
                            {
                                let mut added = false;

                                // Try to add a peer from the right side
                                if right < peers_len {
                                    let peer = &peers[right];
                                    if peer.records_stored.load(Ordering::Relaxed)
                                        < MAX_RECORDS_COUNT
                                    {
                                        close_group.push(right);
                                        added = true;
                                    }
                                    right += 1;
                                }

                                // Try to add a peer from the left side
                                if close_group.len() < replication_group_size && left > 0 {
                                    left -= 1;
                                    let peer = &peers[left];
                                    if peer.records_stored.load(Ordering::Relaxed)
                                        < MAX_RECORDS_COUNT
                                    {
                                        close_group.push(left);
                                        added = true;
                                    }
                                }

                                // If we couldn't add from either side, expand the search range
                                if !added {
                                    if right >= peers_len && left == 0 {
                                        println!("Failed to find a closer");
                                        failed_payee_finding.store(true, Ordering::Relaxed);

                                        // bail!("Failed to find a payee");

                                        // We've exhausted all possibilities
                                        break;
                                    }
                                    continue;
                                }
                            }

                            if close_group.len() < 2 {
                                println!("waning group size: {:?}", close_group.len());
                            }
                            if close_group.len() == 0 {
                                failed_payee_finding.store(true, Ordering::Relaxed);
                                println!("No nodes close_group.len() {:?}!", close_group.len());
                                bail!("No nodes close_group.len() {:?}!", close_group.len());
                            }
                            // Truncate to ensure we have exactly close_group_size peers
                            close_group.sort_by_key(|&index| {
                                sorting_target_address.distance(&peers[index].address)
                            });
                            close_group.truncate(replication_group_size);

                            // Find the cheapest payee among the close group
                            let Ok((payee_index, cost)) = pick_the_payee(&peers, &close_group)
                            else {
                                println!("Failed to find a payee");
                                failed_payee_finding.store(true, Ordering::Relaxed);

                                bail!("Failed to find a payee");
                            };

                            if cost.as_nano() >= MAX_STORE_COST {
                                total_failed_payments.fetch_add(1, Ordering::Relaxed);
                                return Ok(());
                            }

                            for &peer_index in &close_group {
                                let peer = &peers[peer_index];
                                peer.records_stored.fetch_add(1, Ordering::Relaxed);

                                if peer_index == payee_index {
                                    peer.nanos_earned
                                        .fetch_add(cost.as_nano(), Ordering::Relaxed);
                                    peer.payments_received.fetch_add(1, Ordering::Relaxed);
                                }
                            }

                            Ok(())
                        })
                        .collect();

                    // Parallel reduction to calculate statistics
                    let (
                        received_payment_count,
                        empty_earned_nodes,
                        min_earned,
                        max_earned,
                        min_store_cost,
                        max_store_cost,
                    ) = peers
                        .par_iter()
                        .map(|peer| {
                            let cost = calculate_cost_for_records(
                                peer.records_stored.load(Ordering::Relaxed),
                            );
                            let earned = peer.nanos_earned.load(Ordering::Relaxed);
                            (
                                peer.payments_received.load(Ordering::Relaxed),
                                if earned == 0 { 1 } else { 0 },
                                earned,
                                earned,
                                cost,
                                cost,
                            )
                        })
                        .reduce(
                            || (0, 0, u64::MAX, 0, u64::MAX, 0),
                            |a, b| {
                                let (
                                    a_received_payment_count,
                                    a_empty_earned_nodes,
                                    a_min_earned,
                                    a_max_earned,
                                    a_min_store_cost,
                                    a_max_store_cost,
                                ) = a;
                                let (
                                    b_received_payment_count,
                                    b_empty_earned_nodes,
                                    b_min_earned,
                                    b_max_earned,
                                    b_min_store_cost,
                                    b_max_store_cost,
                                ) = b;
                                (
                                    a_received_payment_count + b_received_payment_count,
                                    a_empty_earned_nodes + b_empty_earned_nodes,
                                    a_min_earned.min(b_min_earned),
                                    a_max_earned.max(b_max_earned),
                                    a_min_store_cost.min(b_min_store_cost),
                                    a_max_store_cost.max(b_max_store_cost),
                                )
                            },
                        );

                    println!("After the completion of hour {hour} with {num_of_chunks_per_hour} chunks put, there are {empty_earned_nodes} nodes which earned nothing");
                    println!("\t\t with storecost variation of (min {min_store_cost} - max {max_store_cost}), and earned variation of (min {min_earned} - max {max_earned})");

                    hour += 1;

                    println!("received_payment_count: {received_payment_count}");

                    // Check termination condition
                    if received_payment_count >= max_payments_attempts_made
                        || failed_payee_finding.load(Ordering::Relaxed)
                    {
                        write_simulation_data(
                            &peers,
                            &run_dir,
                            num_of_peers,
                            max_payments_attempts_made,
                            replication_group_size,
                        )?;

                        if total_failed_payments.load(Ordering::Relaxed)
                            >= max_payments_attempts_made / 2
                        {
                            println!("50% of payments failed, stopping simulation");
                            bail!("50% of payments failed, stopping simulation");
                        }

                        println!("received_payment_count: {received_payment_count}");
                        let acceptable_percentage = 0.01; //%

                        // make min earned at least 1
                        let min_earned = min_earned.max(1);

                        // Calculate acceptable empty nodes based on % of total nodes
                        let acceptable_empty_nodes =
                            (num_of_peers as f64 * acceptable_percentage).ceil() as usize;

                        // // Assert conditions for termination
                        // assert!(
                        //         empty_earned_nodes <= acceptable_empty_nodes,
                        //         "More than {acceptable_percentage}% of nodes ({acceptable_empty_nodes}) still not earning: {empty_earned_nodes}"
                        //     );
                        // assert!(
                        //     (max_store_cost / min_store_cost) < 100,
                        //     "store cost is not 'balanced', expected ratio max/min to be < 100, but was {}",
                        //     max_store_cost / min_store_cost
                        // );
                        // assert!(
                        //     (max_earned / min_earned) < 1500,
                        //     "earning distribution is not balanced, expected to be < 1500, but was {}",
                        //     max_earned / min_earned
                        // );
                        break;
                    }
                }
            }
        }
    }

    Ok(())
}

fn create_run_directory() -> eyre::Result<std::path::PathBuf> {
    let temp_dir = std::env::temp_dir();
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let datetime: chrono::DateTime<chrono::Utc> =
        (UNIX_EPOCH + std::time::Duration::from_secs(now)).into();
    let formatted_time = datetime.format("%Y-%m-%d_%H-%M").to_string();
    let run_dir = temp_dir.join(format!("payment_sim_run_{}", formatted_time));
    fs::create_dir_all(&run_dir)?;
    println!("Created run directory: {:?}", run_dir);
    Ok(run_dir)
}

fn write_simulation_data(
    peers: &[PeerStats],
    run_dir: &std::path::Path,
    num_of_peers: usize,
    max_payments: usize,
    replication_group_size: usize,
) -> eyre::Result<()> {
    let graphs_dir = run_dir.join("graphs");
    let peer_data_dir = run_dir.join("peer_data");
    let sim_dir = create_simulation_directory(
        &peer_data_dir,
        num_of_peers,
        max_payments,
        replication_group_size,
    )?;

    write_peers_data_to_file(peers, &sim_dir)?;
    generate_graph(
        peers,
        &graphs_dir,
        num_of_peers,
        max_payments,
        replication_group_size,
    )?;

    if let Some(peer) = peers.iter().max_by(|peer1, peer2| {
        peer1
            .nanos_earned
            .load(Ordering::Relaxed)
            .cmp(&peer2.nanos_earned.load(Ordering::Relaxed))
    }) {
        println!("Largest payee {peer:?}.");
    }
    if let Some(peer) = peers.iter().min_by(|peer1, peer2| {
        peer1
            .nanos_earned
            .load(Ordering::Relaxed)
            .cmp(&peer2.nanos_earned.load(Ordering::Relaxed))
    }) {
        println!("Smallest payee {peer:?}.");
    }
    let output = format!("Run directory: {:?}", run_dir);
    let dashes = "-".repeat(output.len());
    println!("{}", dashes);
    println!("{}", output);
    println!("{}", dashes);

    Ok(())
}

fn create_simulation_directory(
    peer_data_dir: &std::path::Path,
    num_of_peers: usize,
    max_payments: usize,
    replication_group_size: usize,
) -> eyre::Result<std::path::PathBuf> {
    let sim_dir = peer_data_dir.join(format!(
        "peers_{}_payments_{}_replication_{}",
        num_of_peers, max_payments, replication_group_size
    ));
    fs::create_dir_all(&sim_dir)?;
    println!("Created simulation directory: {:?}", sim_dir);
    Ok(sim_dir)
}

/// Write peers data as space separated values to a temp file, and then print the file location
/// sort the peers by max earned
fn write_peers_data_to_file(peers: &[PeerStats], sim_dir: &std::path::Path) -> eyre::Result<()> {
    println!("Writing peers data to a file");
    let file_name = "peers_data.txt";
    let file_path = sim_dir.join(file_name);
    let file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&file_path)?;
    let mut writer = std::io::BufWriter::with_capacity(16 * 1024, file);

    // Preload peer data to minimize atomic loads during sorting and writing
    let mut peers_data: Vec<_> = peers
        .iter()
        .map(|peer| {
            (
                peer.nanos_earned.load(Ordering::Relaxed),
                peer.records_stored.load(Ordering::Relaxed),
                peer.payments_received.load(Ordering::Relaxed),
                peer.address.clone(),
            )
        })
        .collect();

    // Sort peers by nanos_earned in descending order for better cache locality
    peers_data.sort_unstable_by(|a, b| b.0.cmp(&a.0));

    // Write header
    writeln!(
        writer,
        "{:<50} {:<20} {:<20} {:<20} {:<20} {:<20}",
        "PeerId",
        "Records_Stored",
        "MeanPaymentCount",
        "Nanos_Earned",
        "Payments_Received",
        "Distance",
    )?;

    let default_chunk_address =
        NetworkAddress::from_chunk_address(ChunkAddress::new(XorName::default()));
    for (nanos_earned, records_stored, payments_received, address) in peers_data {
        let distance_to_default = address
            .distance(&default_chunk_address)
            .ilog2()
            .unwrap_or(0);
        // It's safe to unwrap here because PeerId should be present
        let peer_id = address.as_peer_id().unwrap();

        let mean_payment_count = nanos_earned as f64 / peers.len() as f64;
        // Use writeln! macro to write formatted line and add newline
        writeln!(
            writer,
            "{:<50} {:<20} {:<20} {:<20} {:<20} {:<10}",
            peer_id,
            records_stored,
            mean_payment_count,
            nanos_earned,
            payments_received,
            distance_to_default,
        )?;
    }

    Ok(())
}

fn generate_graph(
    peers: &[PeerStats],
    graphs_dir: &std::path::Path,
    num_of_peers: usize,
    max_payments: usize,
    replication_group_size: usize,
) -> eyre::Result<()> {
    use plotters::prelude::*;

    let file_name = format!(
        "{}_{}_{}.png",
        replication_group_size, num_of_peers, max_payments,
    );

    let file_path = graphs_dir.join(file_name);
    let root = BitMapBackend::new(file_path.to_str().unwrap(), (1200, 800)).into_drawing_area();
    root.fill(&WHITE)?;

    // Calculate average nanos earned for each peer
    let peers_len = peers.len() as f64;
    let nanos_earned: Vec<u64> = peers
        .iter()
        .map(|peer| peer.nanos_earned.load(Ordering::Relaxed) as u64)
        .collect();

    let min_nanos = nanos_earned.iter().fold(u64::MAX, |a, &b| a.min(b));
    let max_nanos = nanos_earned.iter().fold(u64::MIN, |a, &b| a.max(b));

    // Create fixed buckets for nanos earned
    const NUM_BUCKETS: usize = 100;

    let bucket_size = (max_nanos - min_nanos) / (NUM_BUCKETS as u64 - 1);
    let mut histogram = vec![0_u64; NUM_BUCKETS];
    for &nanos in &nanos_earned {
        let bucket = ((nanos - min_nanos) * (NUM_BUCKETS as u64 - 1) / (max_nanos - min_nanos))
            .min((NUM_BUCKETS - 1) as u64) as usize;
        histogram[bucket] += 1;
    }

    let max_count = *histogram.iter().max().unwrap_or(&0);

    // Determine appropriate y-axis range
    let y_max = if max_count < 10 {
        10
    } else if max_count < 100 {
        ((max_count + 9) / 10) * 10
    } else {
        ((max_count + 99) / 100) * 100
    };

    let mut chart = ChartBuilder::on(&root)
        .caption(
            format!(
                "(Replication Group Size: {}, Peers: {}, Payments: {})",
                replication_group_size, num_of_peers, max_payments,
            ),
            ("sans-serif", 20, FontStyle::Bold).into_font(),
        )
        .margin(50)
        .x_label_area_size(60)
        .y_label_area_size(80)
        .build_cartesian_2d(0..NUM_BUCKETS, 0..y_max)?;

    chart
        .configure_mesh()
        .x_desc(format!("Nanos Earned (nanos/bucket: {bucket_size:?})",))
        .y_desc("Number of Nodes")
        .x_labels(10)
        .x_label_formatter(&|&v| {
            let lower: u64 = v as u64 * bucket_size;
            format!("{:?}", lower)
        })
        .y_labels(10)
        .y_label_formatter(&|v| format!("{}", v))
        .draw()?;

    chart.draw_series(
        Histogram::vertical(&chart)
            .style(BLUE.filled())
            .margin(0)
            .data(histogram.iter().enumerate().map(|(i, &c)| (i, c))),
    )?;

    root.present()?;

    Ok(())
}

fn pick_the_payee(peers: &[PeerStats], close_group: &[usize]) -> eyre::Result<(usize, NanoTokens)> {
    let mut costs_vec = Vec::with_capacity(close_group.len());
    let mut address_to_index = BTreeMap::new();

    for &i in close_group {
        let peer = &peers[i];
        address_to_index.insert(peer.address.clone(), i);

        let close_records_stored = peer.records_stored.load(Ordering::Relaxed);
        let cost: NanoTokens = NanoTokens::from(calculate_cost_for_records(close_records_stored));

        let quote = PaymentQuote {
            content: XorName::default(), // unimportant for cost calc
            cost,
            timestamp: std::time::SystemTime::now(),
            quoting_metrics: QuotingMetrics {
                close_records_stored: peer.records_stored.load(Ordering::Relaxed),
                max_records: MAX_RECORDS_COUNT,
                received_payment_count: 1, // unimportant for cost calc
                live_time: 0,              // unimportant for cost calc
            },
            pub_key: peer.pk.to_bytes().to_vec(),
            signature: vec![], // unimportant for cost calc
        };

        costs_vec.push((peer.address.clone(), peer.pk, quote));
    }

    // sort by address first
    costs_vec.sort_by(|(a_addr, _, _), (b_addr, _, _)| a_addr.cmp(b_addr));

    let Ok((recip_id, _pk, q)) = get_fees_from_store_cost_responses(costs_vec) else {
        bail!("Failed to get fees from store cost responses")
    };

    let Some(index) = address_to_index
        .get(&NetworkAddress::from_peer(recip_id))
        .copied()
    else {
        bail!("Cannot find the index for the cheapest payee");
    };

    Ok((index, q.cost))
}
