// Copyright (C) 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod config;
#[cfg(test)]
mod tests;

pub use config::{AddServiceOptions, UpgradeOptions};

use self::config::InstallNodeServiceCtxBuilder;
use crate::{config::create_owned_dir, service::ServiceControl, VerbosityLevel};
use color_eyre::{
    eyre::{eyre, OptionExt},
    Help, Result,
};
use colored::Colorize;
use libp2p::multiaddr::Protocol;
use semver::Version;
use sn_node_rpc_client::{RpcActions, RpcClient};
use sn_protocol::node_registry::{Node, NodeRegistry, NodeStatus};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

pub enum UpgradeResult {
    Forced(String, String),
    NotRequired,
    Upgraded(String, String),
    Error(String),
}

// macOS seems to require this delay to be in seconds rather than milliseconds.
const RPC_START_UP_DELAY_MS: u64 = 3000;

/// Install safenode as a service.
///
/// This only defines the service; it does not start it.
///
/// There are several arguments that probably seem like they could be handled within the function,
/// but they enable more controlled unit testing.
pub async fn add(
    options: AddServiceOptions,
    node_registry: &mut NodeRegistry,
    service_control: &dyn ServiceControl,
    verbosity: VerbosityLevel,
) -> Result<()> {
    if options.genesis {
        if let Some(count) = options.count {
            if count > 1 {
                return Err(eyre!("A genesis node can only be added as a single node"));
            }
        }

        let genesis_node = node_registry.nodes.iter().find(|n| n.genesis);
        if genesis_node.is_some() {
            return Err(eyre!("A genesis node already exists"));
        }
    }

    if options.count.is_some() && options.node_port.is_some() {
        let count = options.count.unwrap();
        if count > 1 {
            return Err(eyre!(
                "Custom node port can only be used when adding a single service"
            ));
        }
    }

    let safenode_file_name = options
        .safenode_bin_path
        .file_name()
        .ok_or_else(|| eyre!("Could not get filename from the safenode download path"))?
        .to_string_lossy()
        .to_string();

    //  store the bootstrap peers and the provided env variable.
    {
        let mut should_save = false;
        let new_bootstrap_peers: Vec<_> = options
            .bootstrap_peers
            .iter()
            .filter(|peer| !node_registry.bootstrap_peers.contains(peer))
            .collect();
        if !new_bootstrap_peers.is_empty() {
            node_registry
                .bootstrap_peers
                .extend(new_bootstrap_peers.into_iter().cloned());
            should_save = true;
        }

        if options.env_variables.is_some() {
            node_registry.environment_variables = options.env_variables.clone();
            should_save = true;
        }

        if should_save {
            node_registry.save()?;
        }
    }

    let mut added_service_data = vec![];
    let mut failed_service_data = vec![];

    let current_node_count = node_registry.nodes.len() as u16;
    let target_node_count = current_node_count + options.count.unwrap_or(1);

    let mut node_number = current_node_count + 1;
    while node_number <= target_node_count {
        let rpc_free_port = service_control.get_available_port()?;
        let rpc_socket_addr = if let Some(addr) = options.rpc_address {
            SocketAddr::new(IpAddr::V4(addr), rpc_free_port)
        } else {
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), rpc_free_port)
        };

        let service_name = format!("safenode{node_number}");
        let service_data_dir_path = options.service_data_dir_path.join(service_name.clone());
        let service_safenode_path = service_data_dir_path.join(safenode_file_name.clone());
        let service_log_dir_path = options.service_log_dir_path.join(service_name.clone());

        create_owned_dir(service_data_dir_path.clone(), &options.user)?;
        create_owned_dir(service_log_dir_path.clone(), &options.user)?;

        std::fs::copy(
            options.safenode_bin_path.clone(),
            service_safenode_path.clone(),
        )?;
        let install_ctx = InstallNodeServiceCtxBuilder {
            local: options.local,
            data_dir_path: service_data_dir_path.clone(),
            genesis: options.genesis,
            log_dir_path: service_log_dir_path.clone(),
            name: service_name.clone(),
            node_port: options.node_port,
            bootstrap_peers: options.bootstrap_peers.clone(),
            rpc_socket_addr,
            safenode_path: service_safenode_path.clone(),
            service_user: options.user.clone(),
            env_variables: options.env_variables.clone(),
        }
        .execute()?;

        match service_control.install(install_ctx) {
            Ok(()) => {
                added_service_data.push((
                    service_name.clone(),
                    service_safenode_path.to_string_lossy().into_owned(),
                    service_data_dir_path.to_string_lossy().into_owned(),
                    service_log_dir_path.to_string_lossy().into_owned(),
                    rpc_socket_addr,
                ));

                node_registry.nodes.push(Node {
                    genesis: options.genesis,
                    local: options.local,
                    service_name,
                    user: options.user.clone(),
                    number: node_number,
                    rpc_socket_addr,
                    version: options.version.clone(),
                    status: NodeStatus::Added,
                    listen_addr: None,
                    pid: None,
                    peer_id: None,
                    log_dir_path: service_log_dir_path.clone(),
                    data_dir_path: service_data_dir_path.clone(),
                    safenode_path: service_safenode_path,
                    connected_peers: None,
                });
                // We save the node registry for each service because it's possible any number of
                // services could fail to be added.
                node_registry.save()?;
            }
            Err(e) => {
                failed_service_data.push((service_name.clone(), e.to_string()));
            }
        }

        node_number += 1;
    }

    std::fs::remove_file(options.safenode_bin_path)?;

    if !added_service_data.is_empty() {
        println!("Services Added:");
        for install in added_service_data.iter() {
            println!(" {} {}", "✓".green(), install.0);
            if verbosity != VerbosityLevel::Minimal {
                println!("    - Safenode path: {}", install.1);
                println!("    - Data path: {}", install.2);
                println!("    - Log path: {}", install.3);
                println!("    - RPC port: {}", install.4);
            }
        }
        println!("[!] Note: newly added services have not been started");
    }

    if !failed_service_data.is_empty() {
        println!("Failed to add {} service(s):", failed_service_data.len());
        for failed in failed_service_data.iter() {
            println!("{} {}: {}", "✕".red(), failed.0, failed.1);
        }
        return Err(eyre!("Failed to add one or more services")
            .suggestion("However, any services that were successfully added will be usable."));
    }

    Ok(())
}

pub async fn start(
    node: &mut Node,
    service_control: &dyn ServiceControl,
    rpc_client: &dyn RpcActions,
    verbosity: VerbosityLevel,
) -> Result<()> {
    if let NodeStatus::Running = node.status {
        // The last time we checked the service was running, but it doesn't mean it's actually
        // running at this point in time. If it is running, we don't need to do anything. If it
        // stopped because of a fault, we will drop to the code below and attempt to start it
        // again.
        if service_control.is_service_process_running(node.pid.ok_or_eyre("The PID was not set")?) {
            println!("The {} service is already running", node.service_name);
            return Ok(());
        }
    }

    // At this point the service either hasn't been started for the first time or it has been
    // stopped. If it was stopped, it was either intentional or because it crashed.
    if verbosity != VerbosityLevel::Minimal {
        println!("Attempting to start {}...", node.service_name);
    }
    service_control.start(&node.service_name)?;

    // Give the node a little bit of time to start before initiating the node info query.
    service_control.wait(RPC_START_UP_DELAY_MS);
    let node_info = rpc_client.node_info().await?;
    let network_info = rpc_client.network_info().await?;
    node.listen_addr = Some(
        network_info
            .listeners
            .into_iter()
            .map(|addr| addr.with(Protocol::P2p(node_info.peer_id)))
            .collect(),
    );
    node.pid = Some(node_info.pid);
    node.peer_id = Some(node_info.peer_id);
    node.status = NodeStatus::Running;

    println!("{} Started {} service", "✓".green(), node.service_name);
    if verbosity != VerbosityLevel::Minimal {
        println!("  - Peer ID: {}", node_info.peer_id);
        println!("  - Logs: {}", node_info.log_path.to_string_lossy());
    }

    Ok(())
}

pub async fn stop(node: &mut Node, service_control: &dyn ServiceControl) -> Result<()> {
    match node.status {
        NodeStatus::Added => {
            println!(
                "Service {} has not been started since it was installed",
                node.service_name
            );
            Ok(())
        }
        NodeStatus::Removed => Err(eyre!("Service {} has been removed", node.service_name)),
        NodeStatus::Running => {
            let pid = node.pid.ok_or_eyre("The PID was not set")?;
            if service_control.is_service_process_running(pid) {
                println!("Attempting to stop {}...", node.service_name);
                service_control.stop(&node.service_name)?;
                println!(
                    "{} Service {} with PID {} was stopped",
                    "✓".green(),
                    node.service_name,
                    pid
                );
            } else {
                println!(
                    "{} Service {} was already stopped",
                    "✓".green(),
                    node.service_name
                );
            }
            node.pid = None;
            node.status = NodeStatus::Stopped;
            node.connected_peers = None;
            Ok(())
        }
        NodeStatus::Stopped => {
            println!(
                "{} Service {} was already stopped",
                "✓".green(),
                node.service_name
            );
            Ok(())
        }
    }
}

pub async fn status(
    node_registry: &mut NodeRegistry,
    service_control: &dyn ServiceControl,
    detailed_view: bool,
    output_json: bool,
    fail: bool,
) -> Result<()> {
    // Again confirm that services which are marked running are still actually running.
    // If they aren't we'll mark them as stopped.
    for node in &mut node_registry.nodes {
        let rpc_client = RpcClient::from_socket_addr(node.rpc_socket_addr);
        if let NodeStatus::Running = node.status {
            if let Some(pid) = node.pid {
                // First we can try the PID we have now. If there is still a process running with
                // that PID, we know the node is still running.
                if service_control.is_service_process_running(pid) {
                    match rpc_client.network_info().await {
                        Ok(info) => {
                            node.connected_peers = Some(info.connected_peers);
                        }
                        Err(_) => {
                            node.connected_peers = None;
                        }
                    }
                } else {
                    // The process with the PID we had has died at some point. However, if the
                    // service has been configured to restart on failures, it's possible that a new
                    // process has been launched and hence we would have a new PID. We can use the
                    // RPC service to try and retrieve it.
                    match rpc_client.node_info().await {
                        Ok(info) => {
                            node.pid = Some(info.pid);
                        }
                        Err(_) => {
                            // Finally, if there was an error communicating with the RPC client, we
                            // can assume that this node is actually stopped.
                            node.status = NodeStatus::Stopped;
                            node.pid = None;
                        }
                    }
                    match rpc_client.network_info().await {
                        Ok(info) => {
                            node.connected_peers = Some(info.connected_peers);
                        }
                        Err(_) => {
                            node.connected_peers = None;
                        }
                    }
                }
            }
        }
    }

    if output_json {
        let json = serde_json::to_string(&node_registry.nodes)?;
        println!("{json}");
    } else if detailed_view {
        for node in &node_registry.nodes {
            let service_status = format!("{} - {}", node.service_name, format_status(&node.status));
            let banner = "=".repeat(service_status.len());
            println!("{}", banner);
            println!("{service_status}");
            println!("{}", banner);
            println!("Version: {}", node.version);
            println!(
                "Peer ID: {}",
                node.peer_id.map_or("-".to_string(), |p| p.to_string())
            );
            println!("RPC Socket: {}", node.rpc_socket_addr);
            println!("Listen Addresses: {:?}", node.listen_addr);
            println!(
                "PID: {}",
                node.pid.map_or("-".to_string(), |p| p.to_string())
            );
            println!("Data path: {}", node.data_dir_path.to_string_lossy());
            println!("Log path: {}", node.log_dir_path.to_string_lossy());
            println!("Bin path: {}", node.safenode_path.to_string_lossy());
            println!(
                "Connected peers: {}",
                node.connected_peers
                    .as_ref()
                    .map_or("-".to_string(), |p| p.len().to_string())
            );
            println!();
        }
    } else {
        println!(
            "{:<18} {:<52} {:<7} {:>15}",
            "Service Name", "Peer ID", "Status", "Connected Peers"
        );
        let nodes = node_registry
            .nodes
            .iter()
            .filter(|n| n.status != NodeStatus::Removed)
            .collect::<Vec<&Node>>();
        for node in nodes {
            let peer_id = node.peer_id.map_or("-".to_string(), |p| p.to_string());
            let connected_peers = node
                .connected_peers
                .clone()
                .map_or("-".to_string(), |p| p.len().to_string());
            println!(
                "{:<18} {:<52} {:<7} {:>15}",
                node.service_name,
                peer_id,
                format_status(&node.status),
                connected_peers
            );
        }
    }

    if fail
        && node_registry
            .nodes
            .iter()
            .any(|n| n.status != NodeStatus::Running)
    {
        return Err(eyre!("One or more nodes are not in a running state"));
    }

    Ok(())
}

pub async fn remove(
    node: &mut Node,
    service_control: &dyn ServiceControl,
    keep_directories: bool,
) -> Result<()> {
    if let NodeStatus::Running = node.status {
        if service_control.is_service_process_running(
            node.pid
                .ok_or_else(|| eyre!("The PID should be set before the node is removed"))?,
        ) {
            return Err(eyre!("A running node cannot be removed")
                .suggestion("Stop the node first then try again"));
        } else {
            // If the node wasn't actually running, we should give the user an opportunity to
            // check why it may have failed before removing everything.
            node.pid = None;
            node.status = NodeStatus::Stopped;
            return Err(
                eyre!("This node was marked as running but it had actually stopped")
                    .suggestion("You may want to check the logs for errors before removing it")
                    .suggestion("To remove the node, run the command again."),
            );
        }
    }

    service_control.uninstall(&node.service_name)?;

    if !keep_directories {
        std::fs::remove_dir_all(&node.data_dir_path)?;
        std::fs::remove_dir_all(&node.log_dir_path)?;
    }

    node.status = NodeStatus::Removed;

    println!("{} Service {} was removed", "✓".green(), node.service_name);

    Ok(())
}

pub async fn upgrade(
    options: UpgradeOptions,
    node: &mut Node,
    service_control: &dyn ServiceControl,
    rpc_client: &dyn RpcActions,
) -> Result<UpgradeResult> {
    let current_version = Version::parse(&node.version)?;
    if !options.force
        && (current_version == options.target_version || options.target_version < current_version)
    {
        return Ok(UpgradeResult::NotRequired);
    }

    stop(node, service_control).await?;
    std::fs::copy(options.target_safenode_path, &node.safenode_path)?;

    // Install the service again to make sure we re-use the same node port.
    // Windows requires that the service be uninstalled first.
    service_control.uninstall(&node.service_name.clone())?;
    let install_ctx = InstallNodeServiceCtxBuilder {
        local: node.local,
        data_dir_path: node.data_dir_path.clone(),
        genesis: node.genesis,
        name: node.service_name.clone(),
        node_port: node.get_safenode_port(),
        bootstrap_peers: options.bootstrap_peers,
        rpc_socket_addr: node.rpc_socket_addr,
        log_dir_path: node.log_dir_path.clone(),
        safenode_path: node.safenode_path.clone(),
        service_user: node.user.clone(),
        env_variables: options.env_variables.clone(),
    }
    .execute()?;
    service_control.install(install_ctx)?;

    if options.start_node {
        start(node, service_control, rpc_client, VerbosityLevel::Normal).await?;
    }
    node.version = options.target_version.to_string();

    match options.force {
        true => Ok(UpgradeResult::Forced(
            current_version.to_string(),
            options.target_version.to_string(),
        )),
        false => Ok(UpgradeResult::Upgraded(
            current_version.to_string(),
            options.target_version.to_string(),
        )),
    }
}

fn format_status(status: &NodeStatus) -> String {
    match status {
        NodeStatus::Running => "RUNNING".green().to_string(),
        NodeStatus::Stopped => "STOPPED".red().to_string(),
        NodeStatus::Added => "ADDED".yellow().to_string(),
        NodeStatus::Removed => "REMOVED".red().to_string(),
    }
}
