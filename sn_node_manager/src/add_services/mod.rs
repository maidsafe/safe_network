// Copyright (C) 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.
pub mod config;
#[cfg(test)]
mod tests;

use self::config::{
    AddDaemonServiceOptions, AddFaucetServiceOptions, AddNodeServiceOptions,
    InstallFaucetServiceCtxBuilder, InstallNodeServiceCtxBuilder, PortRange,
};
use crate::{config::create_owned_dir, VerbosityLevel, DAEMON_SERVICE_NAME};
use color_eyre::{eyre::eyre, Help, Result};
use colored::Colorize;
use service_manager::ServiceInstallCtx;
use sn_service_management::{
    control::ServiceControl, DaemonServiceData, FaucetServiceData, NodeRegistry, NodeServiceData,
    ServiceStatus,
};
use std::{
    ffi::OsString,
    net::{IpAddr, Ipv4Addr, SocketAddr},
};

/// Install safenode as a service.
///
/// This only defines the service; it does not start it.
///
/// There are several arguments that probably seem like they could be handled within the function,
/// but they enable more controlled unit testing.
pub async fn add_node(
    options: AddNodeServiceOptions,
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

    if let Some(ref port_range) = options.node_port {
        match port_range {
            PortRange::Single(_) => {
                let count = options.count.unwrap_or(1);
                if count != 1 {
                    return Err(eyre!(
                        "The number of services to add ({count}) does not match the number of ports (1)"
                    ));
                }
            }
            PortRange::Range(start, end) => {
                let port_count = end - start + 1;
                let service_count = options.count.unwrap_or(1);
                if port_count != service_count {
                    return Err(eyre!(
                        "The number of services to add ({service_count}) does not match the number of ports ({port_count})"
                    ));
                }
            }
        }
    }

    let safenode_file_name = options
        .safenode_src_path
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
            node_registry
                .environment_variables
                .clone_from(&options.env_variables);
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
    let mut node_port = get_start_port_if_applicable(options.node_port);
    let mut metrics_port = get_start_port_if_applicable(options.metrics_port);
    let mut rpc_port = get_start_port_if_applicable(options.rpc_port);

    while node_number <= target_node_count {
        let rpc_free_port = if let Some(port) = rpc_port {
            port
        } else {
            service_control.get_available_port()?
        };
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
            options.safenode_src_path.clone(),
            service_safenode_path.clone(),
        )?;
        let install_ctx = InstallNodeServiceCtxBuilder {
            bootstrap_peers: options.bootstrap_peers.clone(),
            data_dir_path: service_data_dir_path.clone(),
            env_variables: options.env_variables.clone(),
            genesis: options.genesis,
            home_network: options.home_network,
            local: options.local,
            log_dir_path: service_log_dir_path.clone(),
            metrics_port,
            name: service_name.clone(),
            node_port,
            rpc_socket_addr,
            safenode_path: service_safenode_path.clone(),
            service_user: options.user.clone(),
        }
        .build()?;

        match service_control.install(install_ctx) {
            Ok(()) => {
                added_service_data.push((
                    service_name.clone(),
                    service_safenode_path.to_string_lossy().into_owned(),
                    service_data_dir_path.to_string_lossy().into_owned(),
                    service_log_dir_path.to_string_lossy().into_owned(),
                    rpc_socket_addr,
                ));

                node_registry.nodes.push(NodeServiceData {
                    connected_peers: None,
                    data_dir_path: service_data_dir_path.clone(),
                    genesis: options.genesis,
                    home_network: options.home_network,
                    listen_addr: None,
                    local: options.local,
                    log_dir_path: service_log_dir_path.clone(),
                    number: node_number,
                    reward_balance: None,
                    rpc_socket_addr,
                    peer_id: None,
                    pid: None,
                    safenode_path: service_safenode_path,
                    service_name,
                    status: ServiceStatus::Added,
                    user: options.user.clone(),
                    version: options.version.clone(),
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
        node_port = increment_port_option(node_port);
        metrics_port = increment_port_option(metrics_port);
        rpc_port = increment_port_option(rpc_port);
    }

    if options.delete_safenode_src {
        std::fs::remove_file(options.safenode_src_path)?;
    }

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

/// Install the daemon as a service.
///
/// This only defines the service; it does not start it.
pub fn add_daemon(
    options: AddDaemonServiceOptions,
    node_registry: &mut NodeRegistry,
    service_control: &dyn ServiceControl,
) -> Result<()> {
    if node_registry.daemon.is_some() {
        return Err(eyre!("A safenodemand service has already been created"));
    }

    std::fs::copy(
        options.daemon_src_bin_path.clone(),
        options.daemon_install_bin_path.clone(),
    )?;

    let install_ctx = ServiceInstallCtx {
        args: vec![
            OsString::from("--port"),
            OsString::from(options.port.to_string()),
            OsString::from("--address"),
            OsString::from(options.address.to_string()),
        ],
        contents: None,
        environment: options.env_variables,
        label: DAEMON_SERVICE_NAME.parse()?,
        program: options.daemon_install_bin_path.clone(),
        username: Some(options.user),
        working_directory: None,
    };

    match service_control.install(install_ctx) {
        Ok(()) => {
            let daemon = DaemonServiceData {
                daemon_path: options.daemon_install_bin_path.clone(),
                endpoint: Some(SocketAddr::new(IpAddr::V4(options.address), options.port)),
                pid: None,
                service_name: DAEMON_SERVICE_NAME.to_string(),
                status: ServiceStatus::Added,
                version: options.version,
            };
            node_registry.daemon = Some(daemon);
            println!("Daemon service added {}", "✓".green());
            println!("[!] Note: the service has not been started");
            node_registry.save()?;
            std::fs::remove_file(options.daemon_src_bin_path)?;
            Ok(())
        }
        Err(e) => {
            println!("Failed to add daemon service: {e}");
            Err(e.into())
        }
    }
}

/// Install the faucet as a service.
///
/// This only defines the service; it does not start it.
///
/// There are several arguments that probably seem like they could be handled within the function,
/// but they enable more controlled unit testing.
pub fn add_faucet(
    install_options: AddFaucetServiceOptions,
    node_registry: &mut NodeRegistry,
    service_control: &dyn ServiceControl,
    verbosity: VerbosityLevel,
) -> Result<()> {
    if node_registry.faucet.is_some() {
        return Err(eyre!("A faucet service has already been created"));
    }

    create_owned_dir(
        install_options.service_log_dir_path.clone(),
        &install_options.user,
    )?;

    std::fs::copy(
        install_options.faucet_src_bin_path.clone(),
        install_options.faucet_install_bin_path.clone(),
    )?;

    let install_ctx = InstallFaucetServiceCtxBuilder {
        bootstrap_peers: install_options.bootstrap_peers.clone(),
        env_variables: install_options.env_variables.clone(),
        faucet_path: install_options.faucet_install_bin_path.clone(),
        local: install_options.local,
        log_dir_path: install_options.service_log_dir_path.clone(),
        name: "faucet".to_string(),
        service_user: install_options.user.clone(),
    }
    .build()?;

    match service_control.install(install_ctx) {
        Ok(()) => {
            node_registry.faucet = Some(FaucetServiceData {
                faucet_path: install_options.faucet_install_bin_path.clone(),
                local: false,
                log_dir_path: install_options.service_log_dir_path.clone(),
                pid: None,
                service_name: "faucet".to_string(),
                status: ServiceStatus::Added,
                user: install_options.user.clone(),
                version: install_options.version,
            });
            println!("Faucet service added {}", "✓".green());
            if verbosity != VerbosityLevel::Minimal {
                println!(
                    "  - Bin path: {}",
                    install_options.faucet_install_bin_path.to_string_lossy()
                );
                println!(
                    "  - Data path: {}",
                    install_options.service_data_dir_path.to_string_lossy()
                );
                println!(
                    "  - Log path: {}",
                    install_options.service_log_dir_path.to_string_lossy()
                );
            }
            println!("[!] Note: the service has not been started");
            std::fs::remove_file(install_options.faucet_src_bin_path)?;
            node_registry.save()?;
            Ok(())
        }
        Err(e) => {
            println!("Failed to add faucet service: {e}");
            Err(e.into())
        }
    }
}

fn get_start_port_if_applicable(range: Option<PortRange>) -> Option<u16> {
    if let Some(port) = range {
        match port {
            PortRange::Single(val) => return Some(val),
            PortRange::Range(start, _) => return Some(start),
        }
    }
    None
}

fn increment_port_option(port: Option<u16>) -> Option<u16> {
    if let Some(port) = port {
        let incremented_port = port + 1;
        return Some(incremented_port);
    }
    None
}
