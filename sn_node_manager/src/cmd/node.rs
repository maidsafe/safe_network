// Copyright (C) 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

#![allow(clippy::too_many_arguments)]

use super::{download_and_get_upgrade_bin_path, is_running_as_root, print_upgrade_summary};
use crate::{
    add_services::{
        add_node,
        config::{AddNodeServiceOptions, PortRange},
    },
    config,
    helpers::{download_and_extract_release, get_bin_version},
    refresh_node_registry, status_report, ServiceManager, VerbosityLevel,
};
use color_eyre::{eyre::eyre, Help, Result};
use colored::Colorize;
use libp2p_identity::PeerId;
use semver::Version;
use sn_peers_acquisition::{get_peers_from_args, PeersArgs};
use sn_releases::{ReleaseType, SafeReleaseRepoActions};
use sn_service_management::{
    control::{ServiceControl, ServiceController},
    get_local_node_registry_path,
    rpc::RpcClient,
    NodeRegistry, NodeService, ServiceStatus, UpgradeOptions, UpgradeResult,
};
use sn_transfers::HotWallet;
use std::{io::Write, net::Ipv4Addr, path::PathBuf, str::FromStr};

pub async fn add(
    count: Option<u16>,
    data_dir_path: Option<PathBuf>,
    env_variables: Option<Vec<(String, String)>>,
    local: bool,
    log_dir_path: Option<PathBuf>,
    metrics_port: Option<PortRange>,
    node_port: Option<PortRange>,
    peers: PeersArgs,
    rpc_address: Option<Ipv4Addr>,
    rpc_port: Option<PortRange>,
    src_path: Option<PathBuf>,
    url: Option<String>,
    user: Option<String>,
    version: Option<String>,
    verbosity: VerbosityLevel,
) -> Result<()> {
    if !is_running_as_root() {
        return Err(eyre!("The add command must run as the root user"));
    }

    if verbosity != VerbosityLevel::Minimal {
        println!("=================================================");
        println!("              Add Safenode Services              ");
        println!("=================================================");
        println!("{} service(s) to be added", count.unwrap_or(1));
    }

    let service_user = user.unwrap_or_else(|| "safe".to_string());
    let service_manager = ServiceController {};
    service_manager.create_service_user(&service_user)?;

    let service_data_dir_path = config::get_service_data_dir_path(data_dir_path, &service_user)?;
    let service_log_dir_path =
        config::get_service_log_dir_path(ReleaseType::Safenode, log_dir_path, &service_user)?;

    let mut node_registry = NodeRegistry::load(&config::get_node_registry_path()?)?;
    let release_repo = <dyn SafeReleaseRepoActions>::default_config();

    let (safenode_src_path, version) = if let Some(path) = src_path {
        let version = get_bin_version(&path)?;
        (path, version)
    } else {
        download_and_extract_release(ReleaseType::Safenode, url.clone(), version, &*release_repo)
            .await?
    };

    // Handle the `PeersNotObtained` error to make the `--peer` argument optional for the node
    // manager.
    //
    // Since we don't use the `network-contacts` feature in the node manager, the `PeersNotObtained`
    // error should occur when no `--peer` arguments were used. If the `safenode` binary we're using
    // has `network-contacts` enabled (which is the case for released binaries), it's fine if the
    // service definition doesn't call `safenode` with a `--peer` argument.
    //
    // It's simpler to not use the `network-contacts` feature in the node manager, because it can
    // return a huge peer list, and that's been problematic for service definition files.
    let is_first = peers.first;
    let bootstrap_peers = match get_peers_from_args(peers).await {
        Ok(p) => p,
        Err(e) => match e {
            sn_peers_acquisition::error::Error::PeersNotObtained => Vec::new(),
            _ => return Err(e.into()),
        },
    };

    let options = AddNodeServiceOptions {
        count,
        env_variables,
        genesis: is_first,
        local,
        metrics_port,
        node_port,
        rpc_address,
        rpc_port,
        safenode_src_path,
        safenode_dir_path: service_data_dir_path.clone(),
        service_data_dir_path,
        service_log_dir_path,
        user: service_user,
        version,
        bootstrap_peers,
    };

    add_node(options, &mut node_registry, &service_manager, verbosity).await?;

    node_registry.save()?;

    Ok(())
}

pub async fn balance(
    peer_ids: Vec<String>,
    service_names: Vec<String>,
    verbosity: VerbosityLevel,
) -> Result<()> {
    if verbosity != VerbosityLevel::Minimal {
        println!("=================================================");
        println!("                 Reward Balances                 ");
        println!("=================================================");
    }

    let mut node_registry = NodeRegistry::load(&config::get_node_registry_path()?)?;
    refresh_node_registry(&mut node_registry, &ServiceController {}, true).await?;

    let service_indices = get_services_for_ops(&node_registry, peer_ids, service_names)?;
    if service_indices.is_empty() {
        // This could be the case if all services are at `Removed` status.
        println!("No balances to display");
        return Ok(());
    }

    for &index in &service_indices {
        let node = &mut node_registry.nodes[index];
        let rpc_client = RpcClient::from_socket_addr(node.rpc_socket_addr);
        let service = NodeService::new(node, Box::new(rpc_client));
        let wallet = HotWallet::load_from(&service.service_data.data_dir_path)?;
        println!(
            "{}: {}",
            service.service_data.service_name,
            wallet.balance()
        );
    }
    Ok(())
}

pub async fn remove(
    keep_directories: bool,
    peer_ids: Vec<String>,
    service_names: Vec<String>,
    verbosity: VerbosityLevel,
) -> Result<()> {
    if !is_running_as_root() {
        return Err(eyre!("The remove command must run as the root user"));
    }

    println!("=================================================");
    println!("           Remove Safenode Services              ");
    println!("=================================================");

    let mut node_registry = NodeRegistry::load(&config::get_node_registry_path()?)?;
    refresh_node_registry(&mut node_registry, &ServiceController {}, true).await?;

    let service_indices = get_services_for_ops(&node_registry, peer_ids, service_names)?;
    if service_indices.is_empty() {
        // This could be the case if all services are at `Removed` status.
        println!("No services were eligible for removal");
        return Ok(());
    }

    let mut failed_services = Vec::new();
    for &index in &service_indices {
        let node = &mut node_registry.nodes[index];
        let rpc_client = RpcClient::from_socket_addr(node.rpc_socket_addr);
        let service = NodeService::new(node, Box::new(rpc_client));
        let mut service_manager =
            ServiceManager::new(service, Box::new(ServiceController {}), verbosity.clone());
        match service_manager.remove(keep_directories).await {
            Ok(()) => {
                node_registry.save()?;
            }
            Err(e) => failed_services.push((node.service_name.clone(), e.to_string())),
        }
    }

    summarise_any_failed_ops(failed_services, "remove")
}

pub async fn reset(force: bool, verbosity: VerbosityLevel) -> Result<()> {
    if !is_running_as_root() {
        return Err(eyre!("The reset command must run as the root user"));
    }

    println!("=================================================");
    println!("           Reset Safenode Services               ");
    println!("=================================================");

    if !force {
        println!("WARNING: all safenode services, data, and logs will be removed.");
        println!("Do you wish to proceed? [y/n]");
        std::io::stdout().flush()?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if input.trim().to_lowercase() != "y" {
            println!("Reset aborted");
            return Ok(());
        }
    }

    stop(vec![], vec![], verbosity.clone()).await?;
    remove(false, vec![], vec![], verbosity).await?;

    let node_registry_path = config::get_node_registry_path()?;
    std::fs::remove_file(node_registry_path)?;

    Ok(())
}

pub async fn start(
    interval: u64,
    peer_ids: Vec<String>,
    service_names: Vec<String>,
    verbosity: VerbosityLevel,
) -> Result<()> {
    if !is_running_as_root() {
        return Err(eyre!("The start command must run as the root user"));
    }

    if verbosity != VerbosityLevel::Minimal {
        println!("=================================================");
        println!("             Start Safenode Services             ");
        println!("=================================================");
    }

    let mut node_registry = NodeRegistry::load(&config::get_node_registry_path()?)?;
    refresh_node_registry(&mut node_registry, &ServiceController {}, true).await?;

    let service_indices = get_services_for_ops(&node_registry, peer_ids, service_names)?;
    if service_indices.is_empty() {
        // This could be the case if all services are at `Removed` status.
        println!("No services were eligible to be started");
        return Ok(());
    }

    let mut failed_services = Vec::new();
    for &index in &service_indices {
        let node = &mut node_registry.nodes[index];
        let rpc_client = RpcClient::from_socket_addr(node.rpc_socket_addr);
        let service = NodeService::new(node, Box::new(rpc_client));
        let mut service_manager =
            ServiceManager::new(service, Box::new(ServiceController {}), verbosity.clone());
        std::thread::sleep(std::time::Duration::from_millis(interval));
        match service_manager.start().await {
            Ok(()) => {
                node_registry.save()?;
            }
            Err(e) => failed_services.push((node.service_name.clone(), e.to_string())),
        }
    }

    summarise_any_failed_ops(failed_services, "start")
}

pub async fn status(details: bool, fail: bool, json: bool) -> Result<()> {
    let mut local_node_registry = NodeRegistry::load(&get_local_node_registry_path()?)?;
    if !local_node_registry.nodes.is_empty() {
        if !json {
            println!("=================================================");
            println!("                Local Network                    ");
            println!("=================================================");
        }
        status_report(
            &mut local_node_registry,
            &ServiceController {},
            details,
            json,
            fail,
        )
        .await?;
        local_node_registry.save()?;
        return Ok(());
    }

    let mut node_registry = NodeRegistry::load(&config::get_node_registry_path()?)?;
    if !node_registry.nodes.is_empty() {
        if !json {
            println!("=================================================");
            println!("                Safenode Services                ");
            println!("=================================================");
        }
        status_report(
            &mut node_registry,
            &ServiceController {},
            details,
            json,
            fail,
        )
        .await?;
        node_registry.save()?;
    }
    Ok(())
}

pub async fn stop(
    peer_ids: Vec<String>,
    service_names: Vec<String>,
    verbosity: VerbosityLevel,
) -> Result<()> {
    if !is_running_as_root() {
        return Err(eyre!("The stop command must run as the root user"));
    }

    if verbosity != VerbosityLevel::Minimal {
        println!("=================================================");
        println!("              Stop Safenode Services             ");
        println!("=================================================");
    }

    let mut node_registry = NodeRegistry::load(&config::get_node_registry_path()?)?;
    refresh_node_registry(&mut node_registry, &ServiceController {}, true).await?;

    let service_indices = get_services_for_ops(&node_registry, peer_ids, service_names)?;
    if service_indices.is_empty() {
        // This could be the case if all services are at `Removed` status.
        println!("No services were eligible to be stopped");
        return Ok(());
    }

    let mut failed_services = Vec::new();
    for &index in &service_indices {
        let node = &mut node_registry.nodes[index];
        let rpc_client = RpcClient::from_socket_addr(node.rpc_socket_addr);
        let service = NodeService::new(node, Box::new(rpc_client));
        let mut service_manager =
            ServiceManager::new(service, Box::new(ServiceController {}), verbosity.clone());
        match service_manager.stop().await {
            Ok(()) => {
                node_registry.save()?;
            }
            Err(e) => failed_services.push((node.service_name.clone(), e.to_string())),
        }
    }

    summarise_any_failed_ops(failed_services, "stop")
}

pub async fn upgrade(
    do_not_start: bool,
    force: bool,
    peer_ids: Vec<String>,
    provided_env_variables: Option<Vec<(String, String)>>,
    service_names: Vec<String>,
    url: Option<String>,
    version: Option<String>,
    verbosity: VerbosityLevel,
) -> Result<()> {
    if !is_running_as_root() {
        return Err(eyre!("The upgrade command must run as the root user"));
    }

    if verbosity != VerbosityLevel::Minimal {
        println!("=================================================");
        println!("           Upgrade Safenode Services             ");
        println!("=================================================");
    }

    let (upgrade_bin_path, target_version) =
        download_and_get_upgrade_bin_path(ReleaseType::Safenode, url, version).await?;

    let mut node_registry = NodeRegistry::load(&config::get_node_registry_path()?)?;
    refresh_node_registry(&mut node_registry, &ServiceController {}, true).await?;

    if !force {
        let node_versions = node_registry
            .nodes
            .iter()
            .map(|n| Version::parse(&n.version).map_err(|_| eyre!("Failed to parse Version")))
            .collect::<Result<Vec<Version>>>()?;
        let any_nodes_need_upgraded = node_versions
            .iter()
            .any(|current_version| current_version < &target_version);
        if !any_nodes_need_upgraded {
            println!("{} All nodes are at the latest version", "✓".green());
            return Ok(());
        }
    }

    let service_indices = get_services_for_ops(&node_registry, peer_ids, service_names)?;
    let mut upgrade_summary = Vec::new();

    for &index in &service_indices {
        let node = &mut node_registry.nodes[index];
        let env_variables = if provided_env_variables.is_some() {
            &provided_env_variables
        } else {
            &node_registry.environment_variables
        };
        let options = UpgradeOptions {
            bootstrap_peers: node_registry.bootstrap_peers.clone(),
            env_variables: env_variables.clone(),
            force,
            start_service: !do_not_start,
            target_bin_path: upgrade_bin_path.clone(),
            target_version: target_version.clone(),
        };

        let rpc_client = RpcClient::from_socket_addr(node.rpc_socket_addr);
        let service = NodeService::new(node, Box::new(rpc_client));
        let mut service_manager =
            ServiceManager::new(service, Box::new(ServiceController {}), verbosity.clone());

        match service_manager.upgrade(options).await {
            Ok(upgrade_result) => {
                upgrade_summary.push((
                    service_manager.service.service_data.service_name.clone(),
                    upgrade_result,
                ));
            }
            Err(e) => {
                upgrade_summary.push((
                    node.service_name.clone(),
                    UpgradeResult::Error(format!("Error: {}", e)),
                ));
            }
        }
    }

    node_registry.save()?;
    print_upgrade_summary(upgrade_summary.clone());

    if upgrade_summary.iter().any(|(_, r)| {
        matches!(r, UpgradeResult::Error(_))
            || matches!(r, UpgradeResult::UpgradedButNotStarted(_, _, _))
    }) {
        return Err(eyre!("There was a problem upgrading one or more nodes").suggestion(
            "For any services that were upgraded but did not start, you can attempt to start them \
                again using the 'start' command."));
    }

    Ok(())
}

fn get_services_for_ops(
    node_registry: &NodeRegistry,
    peer_ids: Vec<String>,
    service_names: Vec<String>,
) -> Result<Vec<usize>> {
    let mut service_indices = Vec::new();

    if service_names.is_empty() && peer_ids.is_empty() {
        for node in node_registry.nodes.iter() {
            if let Some(index) = node_registry.nodes.iter().position(|x| {
                x.service_name == node.service_name && x.status != ServiceStatus::Removed
            }) {
                service_indices.push(index);
            }
        }
    } else {
        for name in &service_names {
            if let Some(index) = node_registry
                .nodes
                .iter()
                .position(|x| x.service_name == *name && x.status != ServiceStatus::Removed)
            {
                service_indices.push(index);
            } else {
                return Err(eyre!(format!("No service named '{name}'")));
            }
        }

        for peer_id_str in &peer_ids {
            let peer_id = PeerId::from_str(peer_id_str)?;
            if let Some(index) = node_registry
                .nodes
                .iter()
                .position(|x| x.peer_id == Some(peer_id) && x.status != ServiceStatus::Removed)
            {
                service_indices.push(index);
            } else {
                return Err(eyre!(format!(
                    "Could not find node with peer ID '{}'",
                    peer_id
                )));
            }
        }
    }

    Ok(service_indices)
}

fn summarise_any_failed_ops(failed_services: Vec<(String, String)>, verb: &str) -> Result<()> {
    if !failed_services.is_empty() {
        println!("Failed to {verb} {} service(s):", failed_services.len());
        for failed in failed_services.iter() {
            println!("{} {}: {}", "✕".red(), failed.0, failed.1);
        }
        return Err(eyre!("Failed to {verb} one or more services"));
    }
    Ok(())
}
