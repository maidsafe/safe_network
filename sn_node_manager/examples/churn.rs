// Copyright (C) 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use clap::Parser;
use color_eyre::{
    eyre::{bail, eyre, OptionExt},
    Result,
};
use libp2p::PeerId;
use sn_protocol::test_utils::DeploymentInventory;
use sn_service_management::{
    safenode_manager_proto::{
        safe_node_manager_client::SafeNodeManagerClient, GetStatusRequest,
        NodeServiceRestartRequest,
    },
    ServiceStatus,
};
use std::{collections::BTreeSet, net::SocketAddr, time::Duration};
use tonic::{transport::Channel, Request};

#[derive(Parser, Debug)]
#[clap(name = "Network Churner")]
struct Opt {
    /// Provide the path to the DeploymentInventory file or the name of the deployment.
    /// If the name is provided, we search for the inventory file in the default location.
    #[clap(long)]
    inventory: String,
    /// The interval at which the nodes should be restarted.
    #[clap(long, value_parser = |t: &str| -> Result<Duration> { Ok(t.parse().map(Duration::from_secs)?)}, default_value = "60")]
    interval: Duration,
    /// The number of nodes to restart concurrently per VM.
    #[clap(long, short = 'c', default_value_t = 2)]
    concurrent_churns: usize,
    /// Set to false to restart the node with a different PeerId.
    #[clap(long, default_value_t = false)]
    retain_peer_id: bool,
    /// The number of time each node in the network is restarted.
    #[clap(long, default_value_t = 1)]
    churn_cycles: usize,
}

#[tokio::main]
async fn main() -> Result<()> {
    let opt = Opt::parse();
    let inventory = DeploymentInventory::load_from_str(&opt.inventory)?;

    // We should not restart the genesis node as it is used as the bootstrap peer for all other nodes.
    let genesis_ip = inventory
        .vm_list
        .iter()
        .find_map(|(name, addr)| {
            if name.contains("genesis") {
                Some(*addr)
            } else {
                None
            }
        })
        .ok_or_eyre("Could not get the genesis VM's addr")?;
    let safenodemand_endpoints = inventory
        .safenodemand_endpoints
        .values()
        .filter(|addr| addr.ip() != genesis_ip)
        .cloned()
        .collect::<BTreeSet<_>>();

    let nodes_per_vm = {
        let mut vms_to_ignore = 0;
        inventory.vm_list.iter().for_each(|(name, _addr)| {
            if name.contains("build") || name.contains("genesis") {
                vms_to_ignore += 1;
            }
        });
        // subtract 1 node for genesis. And ignore build & genesis node.
        (inventory.node_count as usize - 1) / (inventory.vm_list.len() - vms_to_ignore)
    };

    let max_concurrent_churns = std::cmp::min(opt.concurrent_churns, nodes_per_vm);
    let mut n_cycles = 0;
    while n_cycles < opt.churn_cycles {
        println!("==== CHURN CYCLE {} ====", n_cycles + 1);
        // churn one VM at a time.
        for daemon_endpoint in safenodemand_endpoints.iter() {
            println!("==== Restarting nodes @ {} ====", daemon_endpoint.ip());
            let mut daemon_client = get_safenode_manager_rpc_client(*daemon_endpoint).await?;
            let nodes_to_churn = get_running_node_list(&mut daemon_client).await?;

            let mut concurrent_churns = 0;
            for (peer_id, node_number) in nodes_to_churn {
                // we don't call restart concurrently as the daemon does not handle concurrent node registry reads/writes.
                restart_node(peer_id, opt.retain_peer_id, &mut daemon_client).await?;

                println!(
                    "safenode-{node_number:?}.service has been restarted. PeerId: {peer_id:?}"
                );

                concurrent_churns += 1;
                if concurrent_churns >= max_concurrent_churns {
                    println!("Sleeping {:?} before churning.", { opt.interval });
                    tokio::time::sleep(opt.interval).await;
                    concurrent_churns = 0;
                }
            }
        }

        n_cycles += 1;
    }

    Ok(())
}

// Return the list of the nodes that are currently running, along with their service number.
pub async fn get_running_node_list(
    daemon_client: &mut SafeNodeManagerClient<Channel>,
) -> Result<Vec<(PeerId, u32)>> {
    let response = daemon_client
        .get_status(Request::new(GetStatusRequest {}))
        .await?;

    let peers = response
        .get_ref()
        .nodes
        .iter()
        .filter_map(|node| {
            if node.status == ServiceStatus::Running as i32 {
                let peer_id = match &node.peer_id {
                    Some(peer_id) => peer_id,
                    None => return Some(Err(eyre!("PeerId has not been set"))),
                };
                match PeerId::from_bytes(peer_id) {
                    Ok(peer_id) => Some(Ok((peer_id, node.number))),
                    Err(err) => Some(Err(err.into())),
                }
            } else {
                None
            }
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(peers)
}

// Restart a remote safenode service by sending a RPC to the safenode manager daemon.
pub async fn restart_node(
    peer_id: PeerId,
    retain_peer_id: bool,
    daemon_client: &mut SafeNodeManagerClient<Channel>,
) -> Result<()> {
    let _response = daemon_client
        .restart_node_service(Request::new(NodeServiceRestartRequest {
            peer_id: peer_id.to_bytes(),
            delay_millis: 0,
            retain_peer_id,
        }))
        .await?;

    Ok(())
}

// Connect to a RPC socket addr with retry
pub async fn get_safenode_manager_rpc_client(
    socket_addr: SocketAddr,
) -> Result<SafeNodeManagerClient<tonic::transport::Channel>> {
    // get the new PeerId for the current NodeIndex
    let endpoint = format!("https://{socket_addr}");
    let mut attempts = 0;
    loop {
        if let Ok(rpc_client) = SafeNodeManagerClient::connect(endpoint.clone()).await {
            break Ok(rpc_client);
        }
        attempts += 1;
        println!("Could not connect to rpc {endpoint:?}. Attempts: {attempts:?}/10");
        tokio::time::sleep(Duration::from_secs(1)).await;
        if attempts >= 10 {
            bail!("Failed to connect to {endpoint:?} even after 10 retries");
        }
    }
}
