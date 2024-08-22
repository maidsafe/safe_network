use std::path::PathBuf;

use sn_node_manager::{add_services::config::PortRange, VerbosityLevel};
use sn_peers_acquisition::PeersArgs;
use tokio::sync::mpsc::UnboundedSender;

use crate::action::{Action, StatusActions};
use color_eyre::eyre::Result;

use crate::connection_mode::ConnectionMode;

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

pub fn maintain_n_running_nodes(args: MaintainNodesArgs) {
    tokio::task::spawn_local(async move {
        if args.run_nat_detection {
            info!("Running nat detection....");
            if let Err(err) = run_nat_detection_process().await {
                error!("Error while running nat detection {err:?}. Registering the error.");
                if let Err(err) = args.action_sender.send(Action::StatusActions(
                    StatusActions::ErrorWhileRunningNatDetection,
                )) {
                    error!("Error while sending action: {err:?}");
                }
            } else {
                info!("Successfully ran nat detection.");
            }
        }

        let auto_set_nat_flags: bool = args.connection_mode == ConnectionMode::Automatic;
        let upnp: bool = args.connection_mode == ConnectionMode::UPnP;
        let home_network: bool = args.connection_mode == ConnectionMode::HomeNetwork;
        let custom_ports: Option<PortRange> = if args.connection_mode == ConnectionMode::CustomPorts
        {
            match args.port_range {
                Some(port_range) => {
                    debug!("Port range to run nodes: {port_range:?}");
                    Some(port_range)
                }
                None => {
                    debug!("Port range not provided. Using default port range.");
                    None
                }
            }
        } else {
            None
        };
        let owner = if args.owner.is_empty() {
            None
        } else {
            Some(args.owner)
        };

        debug!("************");
        debug!(
            "Maintaining {} running nodes with the following args:",
            args.count
        );
        debug!(
            " owner: {:?}, peers_args: {:?}, safenode_path: {:?}",
            owner, args.peers_args, args.safenode_path
        );
        debug!(
            " data_dir_path: {:?}, connection_mode: {:?}",
            args.data_dir_path, args.connection_mode
        );
        debug!(
            " auto_set_nat_flags: {:?}, custom_ports: {:?}, upnp: {}, home_network: {}",
            auto_set_nat_flags, custom_ports, upnp, home_network
        );

        if let Err(err) = sn_node_manager::cmd::node::maintain_n_running_nodes(
            false,
            auto_set_nat_flags,
            120,
            args.count,
            args.data_dir_path,
            true,
            None,
            home_network,
            false,
            None,
            None,
            None,
            custom_ports,
            owner,
            args.peers_args,
            None,
            None,
            args.safenode_path,
            None,
            upnp,
            None,
            None,
            VerbosityLevel::Minimal,
            None,
        )
        .await
        {
            error!(
                "Error while maintaining {:?} running nodes {err:?}",
                args.count
            );
        } else {
            info!("Maintained {} running nodes successfully.", args.count);
        }
        if let Err(err) = args
            .action_sender
            .send(Action::StatusActions(StatusActions::StartNodesCompleted))
        {
            error!("Error while sending action: {err:?}");
        }
    });
}

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

async fn run_nat_detection_process() -> Result<()> {
    sn_node_manager::cmd::nat_detection::run_nat_detection(
        None,
        true,
        None,
        None,
        Some("0.1.0".to_string()),
        VerbosityLevel::Minimal,
    )
    .await?;
    Ok(())
}
