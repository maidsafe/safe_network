use std::path::PathBuf;

use sn_node_manager::VerbosityLevel;
use sn_peers_acquisition::PeersArgs;
use tokio::sync::mpsc::UnboundedSender;

use crate::action::{Action, StatusActions};
use color_eyre::eyre::Result;

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

pub fn maintain_n_running_nodes(
    count: u16,
    owner: String,
    peers_args: PeersArgs,
    run_nat_detection: bool,
    safenode_path: Option<PathBuf>,
    data_dir_path: Option<PathBuf>,
    action_sender: UnboundedSender<Action>,
) {
    tokio::task::spawn_local(async move {
        if run_nat_detection {
            if let Err(err) = run_nat_detection_process().await {
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

        let owner = if owner.is_empty() { None } else { Some(owner) };
        if let Err(err) = sn_node_manager::cmd::node::maintain_n_running_nodes(
            false,
            true,
            120,
            count,
            data_dir_path,
            true,
            None,
            false,
            false,
            None,
            None,
            None,
            None,
            owner,
            peers_args,
            None,
            None,
            safenode_path,
            None,
            false,
            None,
            None,
            VerbosityLevel::Minimal,
            None,
        )
        .await
        {
            error!("Error while maintaining {count:?} running nodes {err:?}");
        } else {
            info!("Maintained {count} running nodes successfully.");
        }
        if let Err(err) =
            action_sender.send(Action::StatusActions(StatusActions::StartNodesCompleted))
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
