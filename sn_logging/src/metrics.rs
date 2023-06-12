// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use serde::Serialize;
use std::time::Duration;
use sysinfo::{self, CpuExt, NetworkExt, Pid, PidExt, ProcessExt, System, SystemExt};
use tracing::{debug, error};

const UPDATE_INTERVAL: Duration = Duration::from_secs(5);
const TO_MB: f32 = 1_000_000.0;

// The following Metrics are collected and logged
#[derive(Debug, Serialize)]
#[allow(dead_code)]
struct Metrics {
    // Number of threads in the system
    physical_cpu_threads: usize,
    // Percentage of CPU used
    system_cpu_usage_percent: f32,
    // Total system memory in MBytes
    system_total_memory_mb: f32,
    // RAM used in MBytes
    system_memory_used_mb: f32,
    // Percentage of RAM used
    system_memory_usage_percent: f32,
    // Network metrics is None if the default network interface cannot be found
    network: Option<NetworkMetrics>,
    // Process metrics is None if the Pid for the process is invalid
    process: Option<ProcessMetrics>,
}

#[derive(Debug, Serialize)]
#[allow(dead_code)]
struct NetworkMetrics {
    // The default network interface
    interface_name: String,
    // Bytes received during UPDATE_INTERVAL
    bytes_received: u64,
    // Bytes transmitted during UPDATE_INTERVAL
    bytes_transmitted: u64,
    // The total MBytes received through the interface
    total_mb_received: f32,
    // The total MBytes transmitted through the interface
    total_mb_transmitted: f32,
}

#[derive(Debug, Serialize)]
#[allow(dead_code)]
struct ProcessMetrics {
    // Percentage of CPU used by the process
    cpu_usage_percent: f32,
    // RAM used in MBytes
    memory_used_mb: f32,
    // Bytes read from disk during UPDATE_INTERVAL
    bytes_read: u64,
    // Bytes written to disk during UPDATE_INTERVAL
    bytes_written: u64,
    // The total MBytes read from disk by the process
    total_mb_read: f32,
    // The total MBytes written to disk by the process
    total_mb_written: f32,
}

// Obtains the system metrics every UPDATE_INTERVAL and logs it.
// The function should be spawned as a task and should be re-run if our main process is restarted.
pub async fn init_metrics(pid: u32) {
    let mut sys = System::new_all();
    let pid = Pid::from_u32(pid);
    let default_interface = default_net::get_default_interface();

    loop {
        refresh_metrics(&mut sys, pid);

        let process = match sys.process(pid) {
            Some(safenode) => {
                let disk_usage = safenode.disk_usage();
                let process = ProcessMetrics {
                    cpu_usage_percent: safenode.cpu_usage(),
                    memory_used_mb: safenode.memory() as f32 / TO_MB,
                    bytes_read: disk_usage.read_bytes,
                    bytes_written: disk_usage.written_bytes,
                    total_mb_read: disk_usage.total_read_bytes as f32 / TO_MB,
                    total_mb_written: disk_usage.total_written_bytes as f32 / TO_MB,
                };
                Some(process)
            }
            None => {
                // safenode with the provided Pid not found
                None
            }
        };

        let network = if let Ok(default_interface) = &default_interface {
            if let Some((interface_name, network_stat)) = sys
                .networks()
                .into_iter()
                .find(|&(interface, _)| interface == &default_interface.name)
            {
                let network = NetworkMetrics {
                    interface_name: interface_name.clone(),
                    bytes_received: network_stat.received(),
                    bytes_transmitted: network_stat.transmitted(),
                    total_mb_received: network_stat.total_received() as f32 / TO_MB,
                    total_mb_transmitted: network_stat.total_transmitted() as f32 / TO_MB,
                };
                Some(network)
            } else {
                // Could not get stats for the default interface
                None
            }
        } else {
            // Could not obtain the default network interface");
            None
        };

        let cpu_stat = sys.global_cpu_info();
        let metrics = Metrics {
            physical_cpu_threads: sys.cpus().len(),
            system_cpu_usage_percent: cpu_stat.cpu_usage(),
            system_total_memory_mb: sys.total_memory() as f32 / TO_MB,
            system_memory_used_mb: sys.used_memory() as f32 / TO_MB,
            system_memory_usage_percent: (sys.used_memory() as f32 / sys.total_memory() as f32)
                * 100.0,
            network,
            process,
        };
        match serde_json::to_string(&metrics) {
            Ok(metrics) => debug!("PID: {} {metrics}, ", std::process::id()),
            Err(err) => error!("Metrics error, could not serialize to JSON {err}"),
        }

        tokio::time::sleep(UPDATE_INTERVAL).await;
    }
}

// Refreshes only the metrics that we interested in.
fn refresh_metrics(sys: &mut System, pid: Pid) {
    sys.refresh_process(pid);
    sys.refresh_memory();
    sys.refresh_networks();
    sys.refresh_cpu();
}
