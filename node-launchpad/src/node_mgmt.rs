use std::path::PathBuf;

use sn_node_manager::{
    add_services::config::PortRange, config::get_node_registry_path, VerbosityLevel,
};
use sn_peers_acquisition::PeersArgs;
use sn_service_management::NodeRegistry;
use tokio::sync::mpsc::UnboundedSender;

use crate::action::{Action, StatusActions};

use crate::connection_mode::ConnectionMode;

use sn_releases::{self, ReleaseType, SafeReleaseRepoActions};

pub const PORT_MAX: u32 = 65535;
pub const PORT_MIN: u32 = 1024;

/// Stop the specified services
pub fn stop_nodes(services: Vec<String>, action_sender: UnboundedSender<Action>) {
    tokio::task::spawn_local(async move {
        if let Err(err) =
            sn_node_manager::cmd::node::stop(vec![], services, VerbosityLevel::Minimal).await
        {
            error!("Error while stopping services {err:?}");
        } else {
            info!("Successfully stopped services");
        }
        if let Err(err) =
            action_sender.send(Action::StatusActions(StatusActions::StopNodesCompleted))
        {
            error!("Error while sending action: {err:?}");
        }
    });
}

pub struct MaintainNodesArgs {
    pub count: u16,
    pub owner: String,
    pub peers_args: PeersArgs,
    pub run_nat_detection: bool,
    pub safenode_path: Option<PathBuf>,
    pub data_dir_path: Option<PathBuf>,
    pub action_sender: UnboundedSender<Action>,
    pub connection_mode: ConnectionMode,
    pub port_range: Option<PortRange>,
}

/// Maintain the specified number of nodes
pub fn maintain_n_running_nodes(args: MaintainNodesArgs) {
    debug!("Maintaining {} nodes", args.count);
    tokio::task::spawn_local(async move {
        if args.run_nat_detection {
            run_nat_detection(&args.action_sender).await;
        }

        let config = prepare_node_config(&args);
        debug_log_config(&config, &args);

        let node_registry = NodeRegistry::load(&get_node_registry_path().unwrap()).unwrap(); //FIXME: unwrap
        let mut used_ports = get_used_ports(&node_registry);
        let (mut current_port, max_port) = get_port_range(&config.custom_ports);

        let nodes_to_add = args.count as i32 - node_registry.nodes.len() as i32;

        if nodes_to_add <= 0 {
            debug!("Scaling down nodes to {}", nodes_to_add);
            scale_down_nodes(&config, args.count).await;
        } else {
            debug!("Scaling up nodes to {}", nodes_to_add);
            add_nodes(
                &config,
                nodes_to_add,
                &mut used_ports,
                &mut current_port,
                max_port,
            )
            .await;
        }

        debug!("Finished maintaining {} nodes", args.count);
        send_completion_action(&args.action_sender);
    });
}

/// Reset all the nodes
pub fn reset_nodes(action_sender: UnboundedSender<Action>, start_nodes_after_reset: bool) {
    tokio::task::spawn_local(async move {
        if let Err(err) = sn_node_manager::cmd::node::reset(true, VerbosityLevel::Minimal).await {
            error!("Error while resetting services {err:?}");
        } else {
            info!("Successfully reset services");
        }
        if let Err(err) =
            action_sender.send(Action::StatusActions(StatusActions::ResetNodesCompleted {
                trigger_start_node: start_nodes_after_reset,
            }))
        {
            error!("Error while sending action: {err:?}");
        }
    });
}

// --- Helper functions ---

struct NodeConfig {
    auto_set_nat_flags: bool,
    upnp: bool,
    home_network: bool,
    custom_ports: Option<PortRange>,
    owner: Option<String>,
    count: u16,
    data_dir_path: Option<PathBuf>,
    peers_args: PeersArgs,
    safenode_path: Option<PathBuf>,
}

/// Run the NAT detection process
async fn run_nat_detection(action_sender: &UnboundedSender<Action>) {
    info!("Running nat detection....");

    let release_repo = <dyn SafeReleaseRepoActions>::default_config();
    let version = match release_repo
        .get_latest_version(&ReleaseType::NatDetection)
        .await
    {
        Ok(v) => {
            info!("Using NAT detection version {}", v.to_string());
            v.to_string()
        }
        Err(err) => {
            info!("No NAT detection release found, using fallback version 0.1.0");
            info!("Error: {err}");
            "0.1.0".to_string()
        }
    };

    if let Err(err) = sn_node_manager::cmd::nat_detection::run_nat_detection(
        None,
        true,
        None,
        None,
        Some(version),
        VerbosityLevel::Minimal,
    )
    .await
    {
        error!("Error while running nat detection {err:?}. Registering the error.");
        if let Err(err) = action_sender.send(Action::StatusActions(
            StatusActions::ErrorWhileRunningNatDetection,
        )) {
            error!("Error while sending action: {err:?}");
        }
    } else {
        info!("Successfully ran nat detection.");
    }
}

fn prepare_node_config(args: &MaintainNodesArgs) -> NodeConfig {
    NodeConfig {
        auto_set_nat_flags: args.connection_mode == ConnectionMode::Automatic,
        upnp: args.connection_mode == ConnectionMode::UPnP,
        home_network: args.connection_mode == ConnectionMode::HomeNetwork,
        custom_ports: if args.connection_mode == ConnectionMode::CustomPorts {
            args.port_range.clone()
        } else {
            None
        },
        owner: if args.owner.is_empty() {
            None
        } else {
            Some(args.owner.clone())
        },
        count: args.count,
        data_dir_path: args.data_dir_path.clone(),
        peers_args: args.peers_args.clone(),
        safenode_path: args.safenode_path.clone(),
    }
}

/// Debug log the node config
fn debug_log_config(config: &NodeConfig, args: &MaintainNodesArgs) {
    debug!("************ STARTING NODE MAINTENANCE ************");
    debug!(
        "Maintaining {} running nodes with the following args:",
        config.count
    );
    debug!(
        " owner: {:?}, peers_args: {:?}, safenode_path: {:?}",
        config.owner, config.peers_args, config.safenode_path
    );
    debug!(
        " data_dir_path: {:?}, connection_mode: {:?}",
        config.data_dir_path, args.connection_mode
    );
    debug!(
        " auto_set_nat_flags: {:?}, custom_ports: {:?}, upnp: {}, home_network: {}",
        config.auto_set_nat_flags, config.custom_ports, config.upnp, config.home_network
    );
}

/// Get the currently used ports from the node registry
fn get_used_ports(node_registry: &NodeRegistry) -> Vec<u16> {
    let used_ports: Vec<u16> = node_registry
        .nodes
        .iter()
        .filter_map(|node| node.node_port)
        .collect();
    debug!("Currently used ports: {:?}", used_ports);
    used_ports
}

/// Get the port range (u16, u16) from the custom ports PortRange
fn get_port_range(custom_ports: &Option<PortRange>) -> (u16, u16) {
    match custom_ports {
        Some(PortRange::Single(port)) => (*port, *port),
        Some(PortRange::Range(start, end)) => (*start, *end),
        None => (PORT_MIN as u16, PORT_MAX as u16),
    }
}

/// Scale down the nodes
async fn scale_down_nodes(config: &NodeConfig, count: u16) {
    info!("No nodes to add");
    match sn_node_manager::cmd::node::maintain_n_running_nodes(
        false,
        config.auto_set_nat_flags,
        120,
        count,
        config.data_dir_path.clone(),
        true,
        None,
        config.home_network,
        false,
        None,
        None,
        None,
        None, // We don't care about the port, as we are scaling down
        config.owner.clone(),
        config.peers_args.clone(),
        None,
        None,
        config.safenode_path.clone(),
        None,
        config.upnp,
        None,
        None,
        VerbosityLevel::Minimal,
        None,
    )
    .await
    {
        Ok(_) => {
            info!("Scaling down to {} nodes", count);
        }
        Err(err) => {
            error!("Error while scaling down to {} nodes: {err:?}", count);
        }
    }
}

/// Add the specified number of nodes
async fn add_nodes(
    config: &NodeConfig,
    mut nodes_to_add: i32,
    used_ports: &mut Vec<u16>,
    current_port: &mut u16,
    max_port: u16,
) {
    let mut retry_count = 0;
    let max_retries = 5;

    while nodes_to_add > 0 && retry_count < max_retries {
        // Find the next available port
        while used_ports.contains(current_port) && *current_port <= max_port {
            *current_port += 1;
        }

        if *current_port > max_port {
            error!("Reached maximum port number. Unable to find an available port.");
            break;
        }

        let port_range = Some(PortRange::Single(*current_port));
        match sn_node_manager::cmd::node::maintain_n_running_nodes(
            false,
            config.auto_set_nat_flags,
            120,
            config.count,
            config.data_dir_path.clone(),
            true,
            None,
            config.home_network,
            false,
            None,
            None,
            None,
            port_range,
            config.owner.clone(),
            config.peers_args.clone(),
            None,
            None,
            config.safenode_path.clone(),
            None,
            config.upnp,
            None,
            None,
            VerbosityLevel::Minimal,
            None,
        )
        .await
        {
            Ok(_) => {
                info!("Successfully added a node on port {}", current_port);
                used_ports.push(*current_port);
                nodes_to_add -= 1;
                *current_port += 1;
                retry_count = 0; // Reset retry count on success
            }
            Err(err) => {
                if err.to_string().contains("is being used by another service") {
                    warn!(
                        "Port {} is being used, retrying with a different port. Attempt {}/{}",
                        current_port,
                        retry_count + 1,
                        max_retries
                    );
                    *current_port += 1;
                    retry_count += 1;
                } else {
                    error!("Error while adding node on port {}: {err:?}", current_port);
                    retry_count += 1;
                }
            }
        }
    }
}

/// Send the completion action
fn send_completion_action(action_sender: &UnboundedSender<Action>) {
    if let Err(err) = action_sender.send(Action::StatusActions(StatusActions::StartNodesCompleted))
    {
        error!("Error while sending action: {err:?}");
    }
}
