use crate::action::{Action, StatusActions};
use crate::connection_mode::ConnectionMode;
use color_eyre::eyre::{eyre, Error};
use color_eyre::Result;
use sn_evm::{EvmNetwork, RewardsAddress};
use sn_node_manager::{
    add_services::config::PortRange, config::get_node_registry_path, VerbosityLevel,
};
use sn_peers_acquisition::PeersArgs;
use sn_releases::{self, ReleaseType, SafeReleaseRepoActions};
use sn_service_management::NodeRegistry;
use std::{path::PathBuf, str::FromStr};
use tokio::runtime::Builder;
use tokio::sync::mpsc::{self, UnboundedSender};
use tokio::task::LocalSet;

pub const PORT_MAX: u32 = 65535;
pub const PORT_MIN: u32 = 1024;

const NODE_ADD_MAX_RETRIES: u32 = 5;

pub const FIXED_INTERVAL: u64 = 60_000;
pub const CONNECTION_TIMEOUT_START: u64 = 120;

pub const NODES_ALL: &str = "NODES_ALL";

#[derive(Debug)]
pub enum NodeManagementTask {
    MaintainNodes {
        args: MaintainNodesArgs,
    },
    ResetNodes {
        start_nodes_after_reset: bool,
        action_sender: UnboundedSender<Action>,
    },
    StopNodes {
        services: Vec<String>,
        action_sender: UnboundedSender<Action>,
    },
    UpgradeNodes {
        args: UpgradeNodesArgs,
    },
    AddNode {
        args: MaintainNodesArgs,
    },
    RemoveNodes {
        services: Vec<String>,
        action_sender: UnboundedSender<Action>,
    },
    StartNode {
        services: Vec<String>,
        action_sender: UnboundedSender<Action>,
    },
}

#[derive(Clone)]
pub struct NodeManagement {
    task_sender: mpsc::UnboundedSender<NodeManagementTask>,
}

impl NodeManagement {
    pub fn new() -> Result<Self> {
        let (send, mut recv) = mpsc::unbounded_channel();

        let rt = Builder::new_current_thread().enable_all().build()?;

        std::thread::spawn(move || {
            let local = LocalSet::new();

            local.spawn_local(async move {
                while let Some(new_task) = recv.recv().await {
                    match new_task {
                        NodeManagementTask::MaintainNodes { args } => {
                            maintain_n_running_nodes(args).await;
                        }
                        NodeManagementTask::ResetNodes {
                            start_nodes_after_reset,
                            action_sender,
                        } => {
                            reset_nodes(action_sender, start_nodes_after_reset).await;
                        }
                        NodeManagementTask::StopNodes {
                            services,
                            action_sender,
                        } => {
                            stop_nodes(services, action_sender).await;
                        }
                        NodeManagementTask::UpgradeNodes { args } => upgrade_nodes(args).await,
                        NodeManagementTask::RemoveNodes {
                            services,
                            action_sender,
                        } => remove_nodes(services, action_sender).await,
                        NodeManagementTask::StartNode {
                            services,
                            action_sender,
                        } => start_nodes(services, action_sender).await,
                        NodeManagementTask::AddNode { args } => add_node(args).await,
                    }
                }
                // If the while loop returns, then all the LocalSpawner
                // objects have been dropped.
            });

            // This will return once all senders are dropped and all
            // spawned tasks have returned.
            rt.block_on(local);
        });

        Ok(Self { task_sender: send })
    }

    /// Send a task to the NodeManagement local set
    /// These tasks will be executed on a different thread to avoid blocking the main thread
    ///
    /// The results are returned via the standard `UnboundedSender<Action>` that is passed to each task.
    ///
    /// If this function returns an error, it means that the task could not be sent to the local set.
    pub fn send_task(&self, task: NodeManagementTask) -> Result<()> {
        self.task_sender
            .send(task)
            .inspect_err(|err| error!("The node management local set is down {err:?}"))
            .map_err(|_| eyre!("Failed to send task to the node management local set"))?;
        Ok(())
    }
}

/// Stop the specified services
async fn stop_nodes(services: Vec<String>, action_sender: UnboundedSender<Action>) {
    if let Err(err) =
        sn_node_manager::cmd::node::stop(None, vec![], services.clone(), VerbosityLevel::Minimal)
            .await
    {
        error!("Error while stopping services {err:?}");
        send_action(
            action_sender,
            Action::StatusActions(StatusActions::ErrorStoppingNodes {
                services,
                raw_error: err.to_string(),
            }),
        );
    } else {
        info!("Successfully stopped services");
        for service in services {
            send_action(
                action_sender.clone(),
                Action::StatusActions(StatusActions::StopNodesCompleted {
                    service_name: service,
                }),
            );
        }
    }
}

#[derive(Debug)]
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
    pub rewards_address: String,
}

/// Maintain the specified number of nodes
async fn maintain_n_running_nodes(args: MaintainNodesArgs) {
    debug!("Maintaining {} nodes", args.count);
    if args.run_nat_detection {
        run_nat_detection(&args.action_sender).await;
    }

    let config = prepare_node_config(&args);
    debug_log_config(&config, &args);

    let node_registry = match load_node_registry(&args.action_sender).await {
        Ok(registry) => registry,
        Err(err) => {
            error!("Failed to load node registry: {:?}", err);
            return;
        }
    };
    let mut used_ports = get_used_ports(&node_registry);
    let (mut current_port, max_port) = get_port_range(&config.custom_ports);

    let nodes_to_add = args.count as i32 - node_registry.nodes.len() as i32;

    if nodes_to_add <= 0 {
        debug!("Scaling down nodes to {}", nodes_to_add);
        scale_down_nodes(&config, args.count).await;
    } else {
        debug!("Scaling up nodes to {}", nodes_to_add);
        add_nodes(
            &args.action_sender,
            &config,
            nodes_to_add,
            &mut used_ports,
            &mut current_port,
            max_port,
        )
        .await;
    }

    debug!("Finished maintaining {} nodes", args.count);
    send_action(
        args.action_sender,
        Action::StatusActions(StatusActions::StartNodesCompleted {
            service_name: NODES_ALL.to_string(),
        }),
    );
}

/// Reset all the nodes
async fn reset_nodes(action_sender: UnboundedSender<Action>, start_nodes_after_reset: bool) {
    if let Err(err) = sn_node_manager::cmd::node::reset(true, VerbosityLevel::Minimal).await {
        error!("Error while resetting services {err:?}");
        send_action(
            action_sender,
            Action::StatusActions(StatusActions::ErrorResettingNodes {
                raw_error: err.to_string(),
            }),
        );
    } else {
        info!("Successfully reset services");
        send_action(
            action_sender,
            Action::StatusActions(StatusActions::ResetNodesCompleted {
                trigger_start_node: start_nodes_after_reset,
            }),
        );
    }
}

#[derive(Debug)]
pub struct UpgradeNodesArgs {
    pub action_sender: UnboundedSender<Action>,
    pub do_not_start: bool,
    pub custom_bin_path: Option<PathBuf>,
    pub force: bool,
    pub peer_ids: Vec<String>,
    pub provided_env_variables: Option<Vec<(String, String)>>,
    pub service_names: Vec<String>,
    pub url: Option<String>,
    pub version: Option<String>,
}

async fn upgrade_nodes(args: UpgradeNodesArgs) {
    // First we stop the Nodes
    if let Err(err) = sn_node_manager::cmd::node::stop(
        None,
        vec![],
        args.service_names.clone(),
        VerbosityLevel::Minimal,
    )
    .await
    {
        error!("Error while stopping services {err:?}");
        send_action(
            args.action_sender.clone(),
            Action::StatusActions(StatusActions::ErrorUpdatingNodes {
                raw_error: err.to_string(),
            }),
        );
    }

    if let Err(err) = sn_node_manager::cmd::node::upgrade(
        0, // will be overwrite by FIXED_INTERVAL
        args.do_not_start,
        args.custom_bin_path,
        args.force,
        Some(FIXED_INTERVAL),
        args.peer_ids,
        args.provided_env_variables,
        args.service_names,
        args.url,
        args.version,
        VerbosityLevel::Minimal,
    )
    .await
    {
        error!("Error while updating services {err:?}");
        send_action(
            args.action_sender,
            Action::StatusActions(StatusActions::ErrorUpdatingNodes {
                raw_error: err.to_string(),
            }),
        );
    } else {
        info!("Successfully updated services");
        send_action(
            args.action_sender,
            Action::StatusActions(StatusActions::UpdateNodesCompleted),
        );
    }
}

async fn remove_nodes(services: Vec<String>, action_sender: UnboundedSender<Action>) {
    // First we stop the nodes
    if let Err(err) =
        sn_node_manager::cmd::node::stop(None, vec![], services.clone(), VerbosityLevel::Minimal)
            .await
    {
        error!("Error while stopping services {err:?}");
        send_action(
            action_sender.clone(),
            Action::StatusActions(StatusActions::ErrorRemovingNodes {
                services: services.clone(),
                raw_error: err.to_string(),
            }),
        );
    }

    if let Err(err) =
        sn_node_manager::cmd::node::remove(false, vec![], services.clone(), VerbosityLevel::Minimal)
            .await
    {
        error!("Error while removing services {err:?}");
        send_action(
            action_sender,
            Action::StatusActions(StatusActions::ErrorRemovingNodes {
                services,
                raw_error: err.to_string(),
            }),
        );
    } else {
        info!("Successfully removed services {:?}", services);
        for service in services {
            send_action(
                action_sender.clone(),
                Action::StatusActions(StatusActions::RemoveNodesCompleted {
                    service_name: service,
                }),
            );
        }
    }
}

async fn add_node(args: MaintainNodesArgs) {
    debug!("Adding node");

    if args.run_nat_detection {
        run_nat_detection(&args.action_sender).await;
    }

    let config = prepare_node_config(&args);

    let node_registry = match load_node_registry(&args.action_sender).await {
        Ok(registry) => registry,
        Err(err) => {
            error!("Failed to load node registry: {:?}", err);
            return;
        }
    };
    let used_ports = get_used_ports(&node_registry);
    let (mut current_port, max_port) = get_port_range(&config.custom_ports);

    while used_ports.contains(&current_port) && current_port <= max_port {
        current_port += 1;
    }

    if current_port > max_port {
        error!("Reached maximum port number. Unable to find an available port.");
        send_action(
            args.action_sender.clone(),
            Action::StatusActions(StatusActions::ErrorAddingNodes {
                raw_error: format!(
                    "When adding a new node we reached maximum port number ({}).\nUnable to find an available port.",
                    max_port
                ),
            }),
        );
    }

    let port_range = Some(PortRange::Single(current_port));
    match sn_node_manager::cmd::node::add(
        false, // auto_restart,
        config.auto_set_nat_flags,
        Some(config.count),
        config.data_dir_path,
        true, // enable_metrics_server,
        None, // env_variables,
        None, // evm_network
        config.home_network,
        false,      // local,
        None,       // log_dir_path,
        None,       // log_format,
        None,       // max_archived_log_files,
        None,       // max_log_files,
        None,       // metrics_port,
        None,       // node_ip,
        port_range, // node_port
        config.owner.clone(),
        config.peers_args.clone(),
        RewardsAddress::from_str(config.rewards_address.as_str()).unwrap(),
        None,                         // rpc_address,
        None,                         // rpc_port,
        config.safenode_path.clone(), // src_path,
        config.upnp,
        None, // url,
        None, // user,
        None, // version,
        VerbosityLevel::Minimal,
    )
    .await
    {
        Err(err) => {
            error!("Error while adding services {err:?}");
            send_action(
                args.action_sender,
                Action::StatusActions(StatusActions::ErrorAddingNodes {
                    raw_error: err.to_string(),
                }),
            );
        }
        Ok(services) => {
            info!("Successfully added services: {:?}", services);
            for service in services {
                send_action(
                    args.action_sender.clone(),
                    Action::StatusActions(StatusActions::AddNodesCompleted {
                        service_name: service,
                    }),
                );
            }
        }
    }
}

async fn start_nodes(services: Vec<String>, action_sender: UnboundedSender<Action>) {
    debug!("Starting node {:?}", services);
    if let Err(err) = sn_node_manager::cmd::node::start(
        CONNECTION_TIMEOUT_START,
        None,
        vec![],
        services.clone(),
        VerbosityLevel::Minimal,
    )
    .await
    {
        error!("Error while starting services {err:?}");
        send_action(
            action_sender,
            Action::StatusActions(StatusActions::ErrorStartingNodes {
                services,
                raw_error: err.to_string(),
            }),
        );
    } else {
        info!("Successfully started services {:?}", services);
        for service in services {
            send_action(
                action_sender.clone(),
                Action::StatusActions(StatusActions::StartNodesCompleted {
                    service_name: service,
                }),
            );
        }
    }
}

// --- Helper functions ---

fn send_action(action_sender: UnboundedSender<Action>, action: Action) {
    if let Err(err) = action_sender.send(action) {
        error!("Error while sending action: {err:?}");
    }
}

/// Load the node registry and handle errors
async fn load_node_registry(
    action_sender: &UnboundedSender<Action>,
) -> Result<NodeRegistry, Error> {
    match get_node_registry_path() {
        Ok(path) => match NodeRegistry::load(&path) {
            Ok(registry) => Ok(registry),
            Err(err) => {
                error!("Failed to load NodeRegistry: {}", err);
                if let Err(send_err) = action_sender.send(Action::StatusActions(
                    StatusActions::ErrorLoadingNodeRegistry {
                        raw_error: err.to_string(),
                    },
                )) {
                    error!("Error while sending action: {}", send_err);
                }
                Err(eyre!("Failed to load NodeRegistry"))
            }
        },
        Err(err) => {
            error!("Failed to get node registry path: {}", err);
            if let Err(send_err) = action_sender.send(Action::StatusActions(
                StatusActions::ErrorGettingNodeRegistryPath {
                    raw_error: err.to_string(),
                },
            )) {
                error!("Error while sending action: {}", send_err);
            }
            Err(eyre!("Failed to get node registry path"))
        }
    }
}

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
    rewards_address: String,
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
        rewards_address: args.rewards_address.clone(),
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
    match sn_node_manager::cmd::node::maintain_n_running_nodes(
        false,
        config.auto_set_nat_flags,
        CONNECTION_TIMEOUT_START,
        count,
        config.data_dir_path.clone(),
        true,
        None,
        Some(EvmNetwork::ArbitrumSepolia),
        config.home_network,
        false,
        None,
        None,
        None,
        None,
        None,
        None,
        None, // We don't care about the port, as we are scaling down
        config.owner.clone(),
        config.peers_args.clone(),
        RewardsAddress::from_str(config.rewards_address.as_str()).unwrap(),
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
    action_sender: &UnboundedSender<Action>,
    config: &NodeConfig,
    mut nodes_to_add: i32,
    used_ports: &mut Vec<u16>,
    current_port: &mut u16,
    max_port: u16,
) {
    let mut retry_count = 0;

    while nodes_to_add > 0 && retry_count < NODE_ADD_MAX_RETRIES {
        // Find the next available port
        while used_ports.contains(current_port) && *current_port <= max_port {
            *current_port += 1;
        }

        if *current_port > max_port {
            error!("Reached maximum port number. Unable to find an available port.");
            send_action(
                action_sender.clone(),
                Action::StatusActions(StatusActions::ErrorScalingUpNodes {
                    raw_error: format!(
                        "Reached maximum port number ({}).\nUnable to find an available port.",
                        max_port
                    ),
                }),
            );
            break;
        }

        let port_range = Some(PortRange::Single(*current_port));
        match sn_node_manager::cmd::node::maintain_n_running_nodes(
            false,
            config.auto_set_nat_flags,
            CONNECTION_TIMEOUT_START,
            config.count,
            config.data_dir_path.clone(),
            true,
            None,
            Some(EvmNetwork::ArbitrumSepolia),
            config.home_network,
            false,
            None,
            None,
            None,
            None,
            None,
            None,
            port_range,
            config.owner.clone(),
            config.peers_args.clone(),
            RewardsAddress::from_str(config.rewards_address.as_str()).unwrap(),
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
                //TODO: We should use concrete error types here instead of string matching (sn_node_manager)
                if err.to_string().contains("is being used by another service") {
                    warn!(
                        "Port {} is being used, retrying with a different port. Attempt {}/{}",
                        current_port,
                        retry_count + 1,
                        NODE_ADD_MAX_RETRIES
                    );
                } else if err
                    .to_string()
                    .contains("Failed to add one or more services")
                    && retry_count >= NODE_ADD_MAX_RETRIES
                {
                    send_action(
                        action_sender.clone(),
                        Action::StatusActions(StatusActions::ErrorScalingUpNodes {
                            raw_error: "When trying to add a node, we failed.\n\
                                 Maybe you ran out of disk space?\n\
                                 Maybe you need to change the port range?"
                                .to_string(),
                        }),
                    );
                } else if err
                    .to_string()
                    .contains("contains a virus or potentially unwanted software")
                    && retry_count >= NODE_ADD_MAX_RETRIES
                {
                    send_action(
                        action_sender.clone(),
                        Action::StatusActions(StatusActions::ErrorScalingUpNodes {
                            raw_error: "When trying to add a node, we failed.\n\
                             You may be running an old version of safenode service?\n\
                             Did you whitelisted safenode and the launchpad?"
                                .to_string(),
                        }),
                    );
                } else {
                    error!("Range of ports to be used {:?}", *current_port..max_port);
                    error!("Error while adding node on port {}: {err:?}", current_port);
                }
                // In case of error, we increase the port and the retry count
                *current_port += 1;
                retry_count += 1;
            }
        }
    }
    if retry_count >= NODE_ADD_MAX_RETRIES {
        send_action(
            action_sender.clone(),
            Action::StatusActions(StatusActions::ErrorScalingUpNodes {
                raw_error: format!(
                    "When trying to start a node, we reached the maximum amount of retries ({}).\n\
                    Could this be a firewall blocking nodes starting or ports on your router already in use?",
                    NODE_ADD_MAX_RETRIES
                ),
            }),
        );
    }
}
