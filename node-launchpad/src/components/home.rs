// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{utils::centered_rect_fixed, Component, Frame};
use crate::{
    action::{Action, HomeActions},
    components::resource_allocation::GB_PER_NODE,
    config::Config,
    mode::{InputMode, Scene},
};
use color_eyre::eyre::{OptionExt, Result};
use ratatui::{prelude::*, widgets::*};
use sn_node_manager::{config::get_node_registry_path, VerbosityLevel};
use sn_peers_acquisition::PeersArgs;
use sn_service_management::{NodeRegistry, NodeServiceData, ServiceStatus};
use std::path::PathBuf;
use tokio::sync::mpsc::UnboundedSender;

const NODE_START_INTERVAL: usize = 10;

pub struct Home {
    /// Whether the component is active right now, capturing keystrokes + draw things.
    active: bool,
    action_sender: Option<UnboundedSender<Action>>,
    config: Config,
    // state
    node_services: Vec<NodeServiceData>,
    node_table_state: TableState,
    allocated_disk_space: usize,
    discord_username: String,
    // Currently the node registry file does not support concurrent actions and thus can lead to
    // inconsistent state. A simple file lock or a db like file would work.
    lock_registry: bool,
    // Peers to pass into nodes for startup
    peers_args: PeersArgs,
    // If path is provided, we don't fetch the binary from the network
    safenode_path: Option<PathBuf>,
}

impl Home {
    pub fn new(
        allocated_disk_space: usize,
        discord_username: &str,
        peers_args: PeersArgs,
        safenode_path: Option<PathBuf>,
    ) -> Result<Self> {
        let mut home = Self {
            peers_args,
            action_sender: Default::default(),
            config: Default::default(),
            active: true,
            node_services: Default::default(),
            allocated_disk_space,
            node_table_state: Default::default(),
            lock_registry: Default::default(),
            discord_username: discord_username.to_string(),
            safenode_path,
        };
        home.load_node_registry_and_update_states()?;

        Ok(home)
    }

    fn get_actions_sender(&self) -> Result<UnboundedSender<Action>> {
        self.action_sender
            .clone()
            .ok_or_eyre("Action sender not registered")
    }

    fn load_node_registry_and_update_states(&mut self) -> Result<()> {
        let node_registry = NodeRegistry::load(&get_node_registry_path()?)?;
        self.node_services = node_registry
            .nodes
            .into_iter()
            .filter(|node| node.status != ServiceStatus::Removed)
            .collect();
        info!(
            "Loaded node registry. Runnign nodes: {:?}",
            self.node_services.len()
        );

        if !self.node_services.is_empty() && self.node_table_state.selected().is_none() {
            self.node_table_state.select(Some(0));
        }

        Ok(())
    }

    fn get_running_nodes(&self) -> Vec<String> {
        self.node_services
            .iter()
            .filter_map(|node| {
                if node.status == ServiceStatus::Running {
                    Some(node.service_name.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    fn select_next_table_item(&mut self) {
        let i = match self.node_table_state.selected() {
            Some(i) => {
                if i >= self.node_services.len() - 1 {
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
                    self.node_services.len() - 1
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

    #[allow(dead_code)]
    fn get_service_name_of_selected_table_item(&self) -> Option<String> {
        let Some(service_idx) = self.node_table_state.selected() else {
            warn!("No item selected from table, not removing anything");
            return None;
        };
        self.node_services
            .get(service_idx)
            .map(|data| data.service_name.clone())
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

    #[allow(clippy::comparison_chain)]
    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::SwitchScene(scene) => match scene {
                Scene::Home => {
                    self.active = true;
                    // make sure we're in navigation mode
                    return Ok(Some(Action::SwitchInputMode(InputMode::Navigation)));
                }
                Scene::DiscordUsernameInputBox | Scene::ResourceAllocationInputBox => {
                    self.active = true
                }
                _ => self.active = false,
            },
            Action::StoreAllocatedDiskSpace(space) => {
                self.allocated_disk_space = space;
            }
            Action::StoreDiscordUserName(username) => {
                self.discord_username = username;
            }
            Action::HomeActions(HomeActions::StartNodes) => {
                if self.lock_registry {
                    error!("Registry is locked. Cannot start node now.");
                    return Ok(None);
                }

                if self.allocated_disk_space == 0 {
                    info!("Disk space not allocated. Ask for input.");
                    return Ok(Some(Action::HomeActions(
                        HomeActions::TriggerResourceAllocationInputBox,
                    )));
                }
                if self.discord_username.is_empty() {
                    info!("Discord username not assigned. Ask for input.");
                    return Ok(Some(Action::HomeActions(
                        HomeActions::TriggerDiscordUsernameInputBox,
                    )));
                }

                let node_count = self.allocated_disk_space / GB_PER_NODE;
                self.lock_registry = true;
                let action_sender = self.get_actions_sender()?;
                info!("Running maintain node count: {node_count:?}");

                maintain_n_running_nodes(
                    node_count as u16,
                    self.discord_username.clone(),
                    self.peers_args.clone(),
                    self.safenode_path.clone(),
                    action_sender,
                );
            }
            Action::HomeActions(HomeActions::StopNodes) => {
                if self.lock_registry {
                    error!("Registry is locked. Cannot stop node now.");
                    return Ok(None);
                }

                let running_nodes = self.get_running_nodes();
                self.lock_registry = true;
                let action_sender = self.get_actions_sender()?;
                info!("Stopping node service: {running_nodes:?}");

                stop_nodes(running_nodes, action_sender);
            }
            Action::HomeActions(HomeActions::ServiceManagerOperationCompleted) => {
                self.lock_registry = false;
                self.load_node_registry_and_update_states()?;
            }
            // todo: should triggers go here? Make distinction between a component + a scene and how they interact.
            Action::HomeActions(HomeActions::TriggerDiscordUsernameInputBox) => {
                return Ok(Some(Action::SwitchScene(Scene::DiscordUsernameInputBox)));
            }
            Action::HomeActions(HomeActions::TriggerResourceAllocationInputBox) => {
                return Ok(Some(Action::SwitchScene(Scene::ResourceAllocationInputBox)));
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
        if !self.active {
            return Ok(());
        }

        // index 0 is reserved for tab
        let layer_zero = Layout::new(
            Direction::Vertical,
            [
                Constraint::Max(1),
                Constraint::Min(3),
                // footer
                Constraint::Max(3),
            ],
        )
        .split(area);
        let popup_area = centered_rect_fixed(25, 3, area);

        // Node List
        let rows: Vec<_> = self
            .node_services
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
        // give green borders if we are running
        let table_border_style = if self.get_running_nodes().len() > 1 {
            Style::default().green()
        } else {
            Style::default()
        };
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
                    .title("Node list")
                    .borders(Borders::ALL)
                    .border_style(table_border_style),
            )
            .highlight_symbol(">");

        f.render_stateful_widget(table, layer_zero[1], &mut self.node_table_state);

        // popup
        if self.lock_registry {
            f.render_widget(Clear, popup_area);
            f.render_widget(
                Paragraph::new("Please wait...")
                    .alignment(Alignment::Center)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_type(BorderType::Double)
                            .border_style(Style::new().bold()),
                    ),
                popup_area,
            );
        }

        Ok(())
    }
}

fn stop_nodes(services: Vec<String>, action_sender: UnboundedSender<Action>) {
    tokio::task::spawn_local(async move {
        if let Err(err) = sn_node_manager::cmd::node::stop(
            vec![],
            services,
            sn_node_manager::VerbosityLevel::Minimal,
        )
        .await
        {
            error!("Error while stopping services {err:?}");
        } else {
            info!("Successfully stopped services");
        }
        if let Err(err) = action_sender.send(Action::HomeActions(
            HomeActions::ServiceManagerOperationCompleted,
        )) {
            error!("Error while sending action: {err:?}");
        }
    });
}

fn maintain_n_running_nodes(
    count: u16,
    owner: String,
    peers_args: PeersArgs,
    safenode_path: Option<PathBuf>,
    action_sender: UnboundedSender<Action>,
) {
    tokio::task::spawn_local(async move {
        if let Err(err) = sn_node_manager::cmd::node::maintain_n_running_nodes(
            false,
            count,
            None,
            None,
            true,
            false,
            None,
            None,
            None,
            None,
            Some(owner),
            peers_args,
            None,
            None,
            safenode_path,
            None,
            true,
            None,
            None,
            VerbosityLevel::Minimal,
            NODE_START_INTERVAL as u64,
        )
        .await
        {
            error!("Error while maintaining {count:?} running nodes {err:?}");
        } else {
            info!("Maintained {count} running nodes successfully.");
        }
        if let Err(err) = action_sender.send(Action::HomeActions(
            HomeActions::ServiceManagerOperationCompleted,
        )) {
            error!("Error while sending action: {err:?}");
        }
    });
}
