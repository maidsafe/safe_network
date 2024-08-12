// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use color_eyre::Result;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use sn_service_management::{NodeServiceData, ServiceStatus};
use std::{path::PathBuf, time::Instant};
use tokio::sync::mpsc::UnboundedSender;

use crate::action::{Action, StatusActions};

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeStats {
    pub forwarded_rewards: u64,
    pub memory_usage_mb: usize,
}

impl NodeStats {
    fn merge(&mut self, other: &NodeStats) {
        self.forwarded_rewards += other.forwarded_rewards;
        self.memory_usage_mb += other.memory_usage_mb;
    }

    pub fn fetch_all_node_stats(nodes: &[NodeServiceData], action_sender: UnboundedSender<Action>) {
        let node_details = nodes
            .iter()
            .filter_map(|node| {
                if node.status == ServiceStatus::Running {
                    if let Some(metrics_port) = node.metrics_port {
                        Some((
                            node.service_name.clone(),
                            metrics_port,
                            node.data_dir_path.clone(),
                        ))
                    } else {
                        error!(
                            "No metrics port found for {:?}. Skipping stat fetch.",
                            node.service_name
                        );
                        None
                    }
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        if !node_details.is_empty() {
            debug!("Fetching stats from {} nodes", node_details.len());
            tokio::task::spawn_local(async move {
                Self::fetch_all_node_stats_inner(node_details, action_sender).await;
            });
        } else {
            debug!("No running nodes to fetch stats from.");
        }
    }

    async fn fetch_all_node_stats_inner(
        node_details: Vec<(String, u16, PathBuf)>,
        action_sender: UnboundedSender<Action>,
    ) {
        let mut stream = futures::stream::iter(node_details)
            .map(|(service_name, metrics_port, data_dir)| async move {
                (
                    Self::fetch_stat_per_node(metrics_port, data_dir).await,
                    service_name,
                )
            })
            .buffer_unordered(5);

        let mut all_node_stats = NodeStats::default();

        while let Some((result, service_name)) = stream.next().await {
            match result {
                Ok(stats) => {
                    all_node_stats.merge(&stats);
                }
                Err(err) => {
                    error!("Error while fetching stats from {service_name:?}: {err:?}");
                }
            }
        }

        if let Err(err) = action_sender.send(Action::StatusActions(
            StatusActions::NodesStatsObtained(all_node_stats),
        )) {
            error!("Error while sending action: {err:?}");
        }
    }

    async fn fetch_stat_per_node(metrics_port: u16, _data_dir: PathBuf) -> Result<NodeStats> {
        let now = Instant::now();

        let body = reqwest::get(&format!("http://localhost:{metrics_port}/metrics"))
            .await?
            .text()
            .await?;
        let lines: Vec<_> = body.lines().map(|s| Ok(s.to_owned())).collect();
        let all_metrics = prometheus_parse::Scrape::parse(lines.into_iter())?;

        let mut stats = NodeStats {
            memory_usage_mb: 0,
            forwarded_rewards: 0,
        };
        for sample in all_metrics.samples.iter() {
            if sample.metric == "sn_networking_process_memory_used_mb" {
                match sample.value {
                    prometheus_parse::Value::Counter(val)
                    | prometheus_parse::Value::Gauge(val)
                    | prometheus_parse::Value::Untyped(val) => {
                        stats.memory_usage_mb = val as usize;
                    }
                    _ => {}
                }
            } else if sample.metric == "sn_node_total_forwarded_rewards" {
                match sample.value {
                    prometheus_parse::Value::Counter(val)
                    | prometheus_parse::Value::Gauge(val)
                    | prometheus_parse::Value::Untyped(val) => {
                        stats.forwarded_rewards = val as u64;
                    }
                    _ => {}
                }
            }
        }
        trace!(
            "Fetched stats from metrics_port {metrics_port:?} in {:?}",
            now.elapsed()
        );
        Ok(stats)
    }
}
