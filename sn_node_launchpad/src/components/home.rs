// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{Component, Frame};
use crate::{
    action::{Action, HomeActions},
    config::Config,
    mode::Scene,
};
use color_eyre::eyre::{OptionExt, Result};
use ratatui::{prelude::*, widgets::*};
use sn_node_manager::config::get_node_registry_path;
use sn_peers_acquisition::PeersArgs;
use sn_service_management::{NodeRegistry, NodeServiceData, ServiceStatus};
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;

pub struct Home {
    action_sender: Option<UnboundedSender<Action>>,
    config: Config,
    // state
    show_scene: bool,
    running_nodes: Vec<NodeServiceData>,
    node_table_state: TableState,
    // Currently the node registry file does not support concurrent actions and thus can lead to
    // inconsistent state. A simple file lock or a db like file would work.
    lock_registry: bool,
}

impl Home {
    pub fn new() -> Result<Self> {
        let mut home = Self {
            action_sender: Default::default(),
            config: Default::default(),
            show_scene: true,
            running_nodes: Default::default(),
            node_table_state: Default::default(),
            lock_registry: Default::default(),
        };
        home.load_node_registry()?;
        if !home.running_nodes.is_empty() {
            home.node_table_state.select(Some(0));
        }
        Ok(home)
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
            Action::SwitchScene(scene) => match scene {
                Scene::Home => self.show_scene = true,
                _ => self.show_scene = false,
            },
            Action::HomeActions(HomeActions::AddNode) => {
                if self.lock_registry {
                    error!("Registry is locked, cannot add node now.");
                    return Ok(None);
                }
                info!("Adding a new node service");

                let peers = PeersArgs::default(); // will fetch from network contacts
                let action_sender = self.get_actions_sender()?;
                self.lock_registry = true;

                tokio::task::spawn_local(async move {
                    if let Err(err) = sn_node_manager::cmd::node::add(
                        None,
                        None,
                        None,
                        true,
                        false,
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
                        error!("Error while adding service {err:?}")
                    } else {
                        info!("Successfully added service");
                    }
                    // todo: need to handle these properly?
                    if let Err(err) =
                        action_sender.send(Action::HomeActions(HomeActions::AddNodeCompleted))
                    {
                        error!("Error while sending action: {err:?}");
                    }
                });
            }
            Action::HomeActions(HomeActions::StartNodes) => {
                if self.lock_registry {
                    error!("Registry is locked. Cannot start node now.");
                    return Ok(None);
                }
                info!("Starting Node service");
                let action_sender = self.get_actions_sender()?;

                self.lock_registry = true;
                let Some(service_name) = self.get_service_name_of_selected_table_item() else {
                    return Ok(None);
                };
                tokio::task::spawn_local(async move {
                    // I think using 1 thread is causing us to block on the below start function and not really
                    // having a chance to set lock_registry = true and draw from that state. Since the update is slow,
                    // the gui looks laggy. Adding a sleep basically puts this to sleep while drawing with the new state.
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    if let Err(err) = sn_node_manager::cmd::node::start(
                        1,
                        vec![],
                        vec![service_name],
                        sn_node_manager::VerbosityLevel::Minimal,
                    )
                    .await
                    {
                        error!("Error while starting services {err:?}");
                    } else {
                        info!("Successfully started services");
                    }
                    if let Err(err) =
                        action_sender.send(Action::HomeActions(HomeActions::StartNodesCompleted))
                    {
                        error!("Error while sending action: {err:?}");
                    }
                });
            }
            Action::HomeActions(HomeActions::StopNode) => {
                if self.lock_registry {
                    error!("Registry is locked. Cannot stop node now.");
                    return Ok(None);
                }
                info!("Stopping node service");
                let action_sender = self.get_actions_sender()?;

                self.lock_registry = true;
                let Some(service_name) = self.get_service_name_of_selected_table_item() else {
                    return Ok(None);
                };
                tokio::task::spawn_local(async move {
                    if let Err(err) = sn_node_manager::cmd::node::stop(
                        vec![],
                        vec![service_name],
                        sn_node_manager::VerbosityLevel::Minimal,
                    )
                    .await
                    {
                        error!("Error while stopping services {err:?}");
                    } else {
                        info!("Successfully stopped services");
                    }
                    if let Err(err) =
                        action_sender.send(Action::HomeActions(HomeActions::StopNodeCompleted))
                    {
                        error!("Error while sending action: {err:?}");
                    }
                });
            }
            Action::HomeActions(HomeActions::RemoveNode) => {
                if self.lock_registry {
                    error!("Registry is locked. Cannot remove node now.");
                    return Ok(None);
                }
                info!("Removing node service");
                let action_sender = self.get_actions_sender()?;

                self.lock_registry = true;
                let Some(service_name) = self.get_service_name_of_selected_table_item() else {
                    return Ok(None);
                };

                tokio::task::spawn_local(async move {
                    if let Err(err) = sn_node_manager::cmd::node::remove(
                        false,
                        vec![],
                        vec![service_name],
                        sn_node_manager::VerbosityLevel::Minimal,
                    )
                    .await
                    {
                        error!("Error while removing services {err:?}");
                    } else {
                        info!("Successfully removed service");
                    }
                    if let Err(err) =
                        action_sender.send(Action::HomeActions(HomeActions::RemoveNodeCompleted))
                    {
                        error!("Error while sending action: {err:?}");
                    }
                });
            }
            Action::HomeActions(HomeActions::AddNodeCompleted)
            | Action::HomeActions(HomeActions::StartNodesCompleted)
            | Action::HomeActions(HomeActions::StopNodeCompleted)
            | Action::HomeActions(HomeActions::RemoveNodeCompleted) => {
                self.lock_registry = false;
                self.load_node_registry()?;
            }
            Action::HomeActions(HomeActions::PreviousTableItem) => {
                self.select_previous_table_item();
            }
            Action::HomeActions(HomeActions::NextTableItem) => {
                self.select_next_table_item();
            }
            _ => {}
        }
        Ok(None)
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        if !self.show_scene {
            return Ok(());
        }

        // index 0 is reserved for tab
        let layer_zero = Layout::new(
            Direction::Vertical,
            [
                Constraint::Max(1),
                Constraint::Min(5),
                Constraint::Min(3),
                Constraint::Max(3),
            ],
        )
        .split(area);
        let popup_area = Self::centered_rect(25, 25, area);

        // top section
        //
        f.render_widget(
            Paragraph::new("").block(
                Block::default()
                    .title("Autonomi Node Status")
                    .borders(Borders::ALL),
            ),
            layer_zero[1],
        );

        // Node List
        let rows: Vec<_> = self
            .running_nodes
            .iter()
            .filter_map(|n| {
                let peer_id = n.peer_id;
                if n.status == ServiceStatus::Removed {
                    return None;
                }
                let service_name = n.service_name.clone();
                let peer_id = peer_id.map(|p| p.to_string()).unwrap_or("-".to_string());
                let status = format!("{:?}", n.status);

                let row = vec![service_name, peer_id, status];
                Some(Row::new(row))
            })
            .collect();

        let widths = [
            Constraint::Max(15),
            Constraint::Min(30),
            Constraint::Max(10),
        ];
        let table = Table::new(rows, widths)
            .column_spacing(2)
            .header(
                Row::new(vec!["Service", "PeerId", "Status"])
                    .style(Style::new().bold())
                    .bottom_margin(1),
            )
            .highlight_style(Style::new().reversed())
            .block(
                Block::default()
                    .title("Running Nodes")
                    .borders(Borders::ALL),
            )
            .highlight_symbol(">");

        f.render_stateful_widget(table, layer_zero[2], &mut self.node_table_state);

        f.render_widget(
            Paragraph::new("[A]dd node, [S]tart node, [K]ill node, [R]emove node, [Q]uit").block(
                Block::default()
                    .title(" Key commands ")
                    .borders(Borders::ALL),
            ),
            layer_zero[3],
        );

        // popup
        if self.lock_registry {
            f.render_widget(Clear, popup_area);
            f.render_widget(
                Paragraph::new("Please wait...")
                    .alignment(Alignment::Center)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .style(Style::default().bg(Color::Reset)),
                    ),
                popup_area,
            );
        }

        Ok(())
    }
}

impl Home {
    fn get_actions_sender(&self) -> Result<UnboundedSender<Action>> {
        self.action_sender
            .clone()
            .ok_or_eyre("Action sender not registered")
    }

    fn load_node_registry(&mut self) -> Result<()> {
        let node_registry = NodeRegistry::load(&get_node_registry_path()?)?;
        self.running_nodes = node_registry
            .nodes
            .into_iter()
            .filter(|node| node.status != ServiceStatus::Removed)
            .collect();
        info!(
            "Loaded node registry. Runnign nodes: {:?}",
            self.running_nodes.len()
        );

        Ok(())
    }

    fn select_next_table_item(&mut self) {
        let i = match self.node_table_state.selected() {
            Some(i) => {
                if i >= self.running_nodes.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.node_table_state.select(Some(i));
    }

    fn select_previous_table_item(&mut self) {
        let i = match self.node_table_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.running_nodes.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.node_table_state.select(Some(i));
    }

    #[allow(dead_code)]
    fn unselect_table_item(&mut self) {
        self.node_table_state.select(None);
    }

    fn get_service_name_of_selected_table_item(&self) -> Option<String> {
        let Some(service_idx) = self.node_table_state.selected() else {
            warn!("No item selected from table, not removing anything");
            return None;
        };
        self.running_nodes
            .get(service_idx)
            .map(|data| data.service_name.clone())
    }

    /// helper function to create a centered rect using up certain percentage of the available rect `r`
    fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
        let popup_layout = Layout::vertical([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

        Layout::horizontal([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
    }
}
