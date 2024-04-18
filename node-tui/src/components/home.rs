use color_eyre::eyre::Result;
use ratatui::{prelude::*, widgets::*};
use sn_peers_acquisition::PeersArgs;
use sn_service_management::{get_local_node_registry_path, NodeRegistry, ServiceStatus};
use tokio::sync::mpsc::UnboundedSender;

use super::{Component, Frame};
use crate::{action::Action, config::Config};

#[derive(Default)]
pub struct Home {
    command_tx: Option<UnboundedSender<Action>>,
    config: Config,
    node_registry: Option<NodeRegistry>,
    // Network Peers
    pub peers_args: PeersArgs,
}

impl Home {
    pub fn new(peers_args: PeersArgs) -> Self {
        Self { peers_args, ..Default::default() }
    }
}

impl Component for Home {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }

    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        self.config = config;
        Ok(())
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::StartNodes => {
                tracing::debug!("STARTING");
                // let local_node_registry = NodeRegistry::load(&get_local_node_registry_path()?)?;
                let peers = self.peers_args.clone();
                tokio::spawn(async {
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
                    )
                    .await
                    {
                        tracing::error!("ERRROR adding {err:?}")
                    }

                    tracing::debug!("added servicionssss");

                    sn_node_manager::cmd::node::start(1, vec![], vec![], sn_node_manager::VerbosityLevel::Minimal)
                });
            },
            Action::Tick => {
                let local_node_registry = NodeRegistry::load(&get_local_node_registry_path()?)?;

                if !local_node_registry.nodes.is_empty() {
                    self.node_registry = Some(local_node_registry);
                } else {
                    self.node_registry = None;
                }
            },
            Action::StartNode => {
                let _local_node_registry = NodeRegistry::load(&get_local_node_registry_path()?)?;
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
        f.render_widget(
            Paragraph::new("TODO: All Node Stats")
                .block(Block::default().title("Autonomi Node Runner").borders(Borders::ALL)),
            home_layout[0],
        );

        if let Some(registry) = &self.node_registry {
            let nodes: Vec<_> =
                registry
                    .to_status_summary()
                    .nodes
                    .iter()
                    .filter_map(|n| {
                        if let ServiceStatus::Running = n.status {
                            n.peer_id.map(|p| p.to_string())
                        } else {
                            None
                        }
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
            Paragraph::new("[S]tart nodes, [Q]uit")
                .block(Block::default().title(" Key commands ").borders(Borders::ALL)),
            home_layout[2],
        );
        Ok(())
    }
}
