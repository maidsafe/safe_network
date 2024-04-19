use color_eyre::eyre::Result;
use ratatui::{prelude::*, widgets::*};
use sn_node_manager::{cmd::node::ProgressType, config::get_node_registry_path};
use sn_peers_acquisition::PeersArgs;
use sn_service_management::{get_local_node_registry_path, NodeRegistry, ServiceStatus};
use tokio::sync::mpsc::{self, UnboundedSender};

use super::{Component, Frame};
use crate::{action::Action, config::Config};

#[derive(Default)]
pub struct Home {
    command_tx: Option<UnboundedSender<Action>>,
    config: Config,
    progress_messages: Vec<ProgressType>,
    node_registry: Option<NodeRegistry>,
    // Network Peers
    pub peers_args: PeersArgs,
}

impl Home {
    pub fn new(peers_args: PeersArgs) -> Self {
        Self { peers_args, ..Default::default() }
    }

    // TODO: we should have a helper to correctlt choose the registry
    pub fn check_for_node_registry(&mut self) -> Result<()> {
        if self.node_registry.is_none() {
            tracing::debug!("No registryllocal yet...");
            let reg = NodeRegistry::load(&get_local_node_registry_path()?)?;

            // register this
            if !reg.nodes.is_empty() {
                self.node_registry = Some(reg);
            }
        }

        // local nodes failed, so we try the _other_ setup
        if self.node_registry.is_none() {
            tracing::debug!("No registry yet...");
            let reg = NodeRegistry::load(&get_node_registry_path()?)?;
            // register this
            if !reg.nodes.is_empty() {
                self.node_registry = Some(reg);
            }
        }

        Ok(())
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
        let action_sender = self.command_tx.clone();
        match action {
            Action::AddNode => {
                tracing::debug!("adding");

                if let Some(reg) = &self.node_registry {
                    tracing::debug!("No nodes yet...");

                    if reg.nodes.is_empty() {
                        let peers = self.peers_args.clone();

                        // report progress via forwarding messages as actions
                        let (progress_sender, mut progress_receiver) = mpsc::channel::<ProgressType>(1);

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
                                tracing::error!("ERRROR adding {err:?}")
                            }
                        });

                        tracing::debug!("OKAY AND NOW LISTENING TO EVENTS>.....");

                        let action_sender = self.command_tx.clone();
                        tokio::task::spawn(async move {
                            while let Some(report) = progress_receiver.recv().await {
                                tracing::debug!("REPORTTTT {report:?}");
                                if let Some(ref sender) = action_sender {
                                    if let Err(error) = sender.send(Action::ProgressMessage(report)) {
                                        tracing::error!("Err sending progress action: {error:?}");
                                    };
                                }
                            }
                        });

                        tracing::debug!("added servicionssss");
                    } else {
                        tracing::debug!("We've no node registery... so skip the addition of services...");
                    }
                }
            },
            Action::StartNodes => {
                tracing::debug!("STARTING");

                tokio::task::spawn_local(async {
                    sn_node_manager::cmd::node::start(1, vec![], vec![], sn_node_manager::VerbosityLevel::Minimal).await
                });
            },
            Action::ProgressMessage(message) => self.progress_messages.push(message),
            Action::Tick => {
                self.check_for_node_registry()?;
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
        // let text = Text::raw(content)
        //
        let text: String = self.progress_messages.iter().map(|progress| format!("{progress:?}\n")).collect();
        f.render_widget(
            Paragraph::new(text).block(Block::default().title("Autonomi Node Status").borders(Borders::ALL)),
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
