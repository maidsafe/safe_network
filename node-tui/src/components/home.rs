use color_eyre::eyre::Result;
use ratatui::{prelude::*, widgets::*};
use sn_node_manager::{cmd::node::ProgressType, config::get_node_registry_path};
use sn_peers_acquisition::PeersArgs;
use sn_service_management::{NodeRegistry, ServiceStatus};
use tokio::sync::mpsc::{self, UnboundedSender};

use super::{Component, Frame};
use crate::{action::Action, config::Config};

#[derive(Default)]
pub struct Home {
    command_tx: Option<UnboundedSender<Action>>,
    config: Config,
    // state
    node_registry: Option<NodeRegistry>,

    // Network Peers
    pub peers_args: PeersArgs,
}

impl Home {
    pub fn new(peers_args: PeersArgs) -> Result<Self> {
        tracing::debug!("Loading node registry");

        let node_registry = NodeRegistry::load(&get_node_registry_path()?)?;

        Ok(Self { peers_args, node_registry: Some(node_registry), ..Default::default() })
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
            Action::AddNode => {
                tracing::debug!("adding");

                let peers = self.peers_args.clone();
                let (progress_sender, _) = mpsc::channel::<ProgressType>(1);

                tokio::task::spawn_local(async {
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
                        tracing::error!("Error while adding service {err:?}")
                    }
                });

                tracing::debug!("added servicionssss");
            },
            Action::StartNodes => {
                tracing::debug!("STARTING");

                tokio::task::spawn_local(async {
                    sn_node_manager::cmd::node::start(1, vec![], vec![], sn_node_manager::VerbosityLevel::Minimal).await
                });
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
                    tracing::debug!("peer_id {:?} {:?}", peer_id, n.status);
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
