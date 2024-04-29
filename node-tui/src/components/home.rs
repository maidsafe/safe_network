use color_eyre::eyre::{OptionExt, Result};
use ratatui::{prelude::*, widgets::*};
use sn_node_manager::{cmd::node::ProgressType, config::get_node_registry_path};
use sn_peers_acquisition::PeersArgs;
use sn_service_management::{NodeRegistry, ServiceStatus};
use tokio::sync::mpsc::{self, UnboundedSender};

use super::{Component, Frame};
use crate::{
    action::{Action, HomeActions},
    config::Config,
};

#[derive(Default)]
pub struct Home {
    action_sender: Option<UnboundedSender<Action>>,
    config: Config,
    // state
    node_registry: Option<NodeRegistry>,
    // Currently the node registry file does not support concurrent actions and thus can lead to
    // inconsistent state. A simple file lock or a db like file would work.
    lock_registry: bool,

    // Network Peers
    pub peers_args: PeersArgs,
}

impl Home {
    pub fn new(peers_args: PeersArgs) -> Result<Self> {
        debug!("Loading node registry");

        let node_registry = NodeRegistry::load(&get_node_registry_path()?)?;

        Ok(Self { peers_args, node_registry: Some(node_registry), ..Default::default() })
    }
}

impl Component for Home {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.action_sender = Some(tx);
        Ok(())
    }

    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        self.config = config;
        Ok(())
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::HomeActions(HomeActions::AddNode) => {
                if self.lock_registry {
                    error!("Registry is locked, cannot add node now.");
                    return Ok(None);
                }
                info!("Adding a new node service");

                let peers = self.peers_args.clone();
                let (progress_sender, _) = mpsc::channel::<ProgressType>(1);
                let action_sender = self.get_actions_sender()?;
                self.lock_registry = true;

                tokio::task::spawn_local(async move {
                    if let Err(err) = sn_node_manager::cmd::node::add(
                        None,
                        None,
                        None,
                        true,
                        None,
                        None,
                        None,
                        peers,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        sn_node_manager::VerbosityLevel::Minimal,
                        progress_sender,
                    )
                    .await
                    {
                        error!("Error while adding service {err:?}")
                    }
                    info!("Successfully added service");
                    // todo: need to handle these properly?
                    if let Err(err) = action_sender.send(Action::HomeActions(HomeActions::AddNodeCompleted)) {
                        error!("Error while sending action: {err:?}");
                    }
                });
            },
            Action::HomeActions(HomeActions::StartNodes) => {
                if self.lock_registry {
                    error!("Registry is locked. Cannot start node now.");
                    return Ok(None);
                }
                info!("Starting Node service");
                let action_sender = self.get_actions_sender()?;

                self.lock_registry = true;
                tokio::task::spawn_local(async move {
                    if let Err(err) =
                        sn_node_manager::cmd::node::start(1, vec![], vec![], sn_node_manager::VerbosityLevel::Minimal)
                            .await
                    {
                        error!("Error while starting services {err:?}");
                    }
                    if let Err(err) = action_sender.send(Action::HomeActions(HomeActions::StartNodesCompleted)) {
                        error!("Error while sending action: {err:?}");
                    }
                    info!("Successfully started services");
                });
            },
            Action::HomeActions(HomeActions::StopNode) => {
                if self.lock_registry {
                    error!("Registry is locked. Cannot stop node now.");
                    return Ok(None);
                }
                info!("Stopping node service");
                let action_sender = self.get_actions_sender()?;

                self.lock_registry = true;
                tokio::task::spawn_local(async move {
                    if let Err(err) =
                        sn_node_manager::cmd::node::stop(vec![], vec![], sn_node_manager::VerbosityLevel::Minimal).await
                    {
                        error!("Error while stopping services {err:?}");
                    }
                    if let Err(err) = action_sender.send(Action::HomeActions(HomeActions::StopNodeCompleted)) {
                        error!("Error while sending action: {err:?}");
                    }
                    info!("Successfully stopped services");
                });
            },
            Action::HomeActions(HomeActions::AddNodeCompleted)
            | Action::HomeActions(HomeActions::StartNodesCompleted)
            | Action::HomeActions(HomeActions::StopNodeCompleted) => {
                self.lock_registry = false;
                let node_registry = NodeRegistry::load(&get_node_registry_path()?)?;
                self.node_registry = Some(node_registry);
            },
            _ => {},
        }
        Ok(None)
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        // basic home layout
        let home_layout =
            Layout::new(Direction::Vertical, [Constraint::Min(5), Constraint::Min(3), Constraint::Max(3)]).split(area);

        // top section
        //
        f.render_widget(
            Paragraph::new("None").block(Block::default().title("Autonomi Node Status").borders(Borders::ALL)),
            home_layout[0],
        );

        if let Some(registry) = &self.node_registry {
            let nodes: Vec<_> = registry
                .to_status_summary()
                .nodes
                .iter()
                .filter_map(|n| {
                    let peer_id = n.peer_id;
                    debug!("peer_id {:?} {:?}", peer_id, n.status);
                    if n.status == ServiceStatus::Removed {
                        return None;
                    }

                    let id = peer_id.map(|p| p.to_string()).unwrap_or("Pending...".to_string());
                    Some(format!("{id:?}: {:?}", n.status))
                })
                .collect();

            if !nodes.is_empty() {
                let list = List::new(nodes);

                f.render_widget(
                    list.block(Block::default().title("Running nodes").borders(Borders::ALL)),
                    home_layout[1],
                );
            }
        } else {
            f.render_widget(
                Paragraph::new("No nodes running")
                    .block(Block::default().title("Autonomi Node Runner").borders(Borders::ALL)),
                home_layout[1],
            )
        }

        f.render_widget(
            Paragraph::new("[A]dd node, [S]tart node, [Q]uit")
                .block(Block::default().title(" Key commands ").borders(Borders::ALL)),
            home_layout[2],
        );
        Ok(())
    }
}

impl Home {
    fn get_actions_sender(&self) -> Result<UnboundedSender<Action>> {
        self.action_sender.clone().ok_or_eyre("Action sender not registered")
    }
}
