// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use color_eyre::Result;
use regex::Regex;
use std::{
    fs::File,
    io::prelude::*,
    io::BufReader,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
};
use tokio::time::{sleep, Duration};
use walkdir::WalkDir;

const LOG_FILENAME_PREFIX: &str = "safenode.log";

#[derive(Debug)]
struct NodeInfo {
    pid: u32,
    name: String,
    addr: SocketAddr,
    log_path: PathBuf,
}

pub async fn run(logs_path: &Path, node_count: u32, nodes_launch_interval: u64) -> Result<()> {
    sleep(Duration::from_millis(nodes_launch_interval)).await;
    println!();
    println!("======== Verifying nodes ========");

    let expected_node_count = node_count as usize + 1; // we'll also check genesis node
    println!(
        "Checking nodes log files to verify all ({expected_node_count}) nodes \
        have joined. Logs path: {}",
        logs_path.display()
    );
    let mut nodes = nodes_info_from_logs(logs_path)?;

    println!("Number of nodes: {}", nodes.len());
    assert_eq!(
        expected_node_count,
        nodes.len(),
        "Unexpected number of joined nodes. Expected {}, we have {}",
        expected_node_count,
        nodes.len()
    );

    println!("All nodes have joined. Nodes IPs and names:");
    nodes.sort_by(|a, b| a.log_path.cmp(&b.log_path));
    for node in &nodes {
        println!(
            "{} {:>16} -> {} @ {}",
            node.pid,
            node.addr,
            node.name,
            node.log_path.display()
        );
    }
    println!();

    Ok(())
}

// Parse node logs files and extract info for each of them
fn nodes_info_from_logs(path: &Path) -> Result<Vec<NodeInfo>> {
    let mut nodes = Vec::<NodeInfo>::new();
    let re = Regex::new(
        r"Node PID: (\d+),.*name: (.{6}\(\d{8}\)..),.*connection info: ((127\.0\.0\.1|0\.0\.0\.0):\d{5})",
    )?;

    let log_files = WalkDir::new(path).into_iter().filter_map(|entry| {
        entry.ok().and_then(|f| {
            if f.file_type().is_file() {
                Some(f.into_path())
            } else {
                None
            }
        })
    });

    for file_path in log_files {
        let file_name = if let Some(name) = file_path.file_name().and_then(|s| s.to_str()) {
            name
        } else {
            println!("Failed to obtain filename from {}", file_path.display());
            continue;
        };

        if file_name.starts_with(LOG_FILENAME_PREFIX) {
            let file = File::open(&file_path)?;
            let lines = BufReader::new(file).lines();
            lines.filter_map(|item| item.ok()).for_each(|line| {
                if let Some(cap) = re.captures_iter(&line).next() {
                    let mut addr: SocketAddr = cap[3].parse().unwrap();
                    if addr.ip() == IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)) {
                        addr.set_ip(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)));
                    }
                    nodes.push(NodeInfo {
                        pid: cap[1].parse().unwrap(),
                        name: cap[2].to_string(),
                        addr,
                        log_path: file_path.parent().unwrap().to_path_buf(),
                    });
                }
            });
        }
    }

    Ok(nodes)
}
