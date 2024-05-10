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
use sn_node_manager::config::get_node_registry_path;
use sn_peers_acquisition::PeersArgs;
use sn_service_management::{NodeRegistry, NodeServiceData, ServiceStatus};
use std::time::Duration;
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
    // Currently the node registry file does not support concurrent actions and thus can lead to
    // inconsistent state. A simple file lock or a db like file would work.
    lock_registry: bool,
}

impl Home {
    pub fn new(allocated_disk_space: usize) -> Result<Self> {
        let mut home = Self {
            action_sender: Default::default(),
            config: Default::default(),
            active: true,
            node_services: Default::default(),
            allocated_disk_space,
            node_table_state: Default::default(),
            lock_registry: Default::default(),
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

    /// Gets both the Added/Stopped nodes
    fn get_inactive_nodes(&self) -> Vec<String> {
        self.node_services
            .iter()
            .filter_map(|node| {
                if node.status == ServiceStatus::Stopped || node.status == ServiceStatus::Added {
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
            Action::HomeActions(HomeActions::StartNodes) => {
                if self.lock_registry {
                    error!("Registry is locked. Cannot start node now.");
                    return Ok(None);
                }

                if self.allocated_disk_space == 0 {
                    info!("Disk space not allocated, ask for input");
                    // trigger resource allocation if not set
                    return Ok(Some(Action::HomeActions(
                        HomeActions::TriggerResourceAllocationInputBox,
                    )));
                }
                let node_count = self.allocated_disk_space / GB_PER_NODE;
                let running_nodes = self.get_running_nodes();

                if running_nodes.len() > node_count {
                    // stop some nodes
                    let to_stop_count = running_nodes.len() - node_count;
                    let nodes_to_stop = running_nodes
                        .into_iter()
                        .take(to_stop_count)
                        .collect::<Vec<_>>();

                    info!(
                        ?node_count,
                        ?to_stop_count,
                        "We are stopping these services: {nodes_to_stop:?}"
                    );

                    let action_sender = self.get_actions_sender()?;
                    self.lock_registry = true;
                    stop_nodes(nodes_to_stop, action_sender);
                } else if running_nodes.len() < node_count {
                    // run some nodes
                    let to_start_count = node_count - running_nodes.len();

                    let inactive_nodes = self.get_inactive_nodes();

                    let action_sender = self.get_actions_sender()?;
                    self.lock_registry = true;

                    if to_start_count > inactive_nodes.len() {
                        // add + start nodes
                        let to_add_count = to_start_count - inactive_nodes.len();

                        info!(
                            ?node_count,
                            ?to_add_count,
                            "We are adding+starting {to_add_count:?} nodes + starting these services: {inactive_nodes:?}"
                        );
                        add_and_start_nodes(to_add_count, inactive_nodes, action_sender);
                    } else {
                        // start these nodes
                        let nodes_to_start =
                            inactive_nodes.into_iter().take(to_start_count).collect();
                        info!(
                            ?node_count,
                            ?to_start_count,
                            "We are starting these pre-existing services: {nodes_to_start:?}"
                        );
                        start_nodes(nodes_to_start, action_sender)
                    }
                } else {
                    info!("We already have the correct number of nodes");
                }
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
                Constraint::Min(5),
                Constraint::Min(3),
                // footer
                Constraint::Max(3),
            ],
        )
        .split(area);
        let popup_area = centered_rect_fixed(25, 3, area);

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

        f.render_stateful_widget(table, layer_zero[2], &mut self.node_table_state);

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

fn start_nodes(services: Vec<String>, action_sender: UnboundedSender<Action>) {
    tokio::task::spawn_local(async move {
        // I think using 1 thread is causing us to block on the below start function and not really
        // having a chance to set lock_registry = true and draw from that state. Since the update is slow,
        // the gui looks laggy. Adding a sleep basically puts this to sleep while drawing with the new state.
        tokio::time::sleep(Duration::from_millis(100)).await;
        if let Err(err) = sn_node_manager::cmd::node::start(
            NODE_START_INTERVAL as u64,
            vec![],
            services,
            sn_node_manager::VerbosityLevel::Minimal,
        )
        .await
        {
            error!("Error while starting services {err:?}");
        } else {
            info!("Successfully started services");
        }
        if let Err(err) = action_sender.send(Action::HomeActions(
            HomeActions::ServiceManagerOperationCompleted,
        )) {
            error!("Error while sending action: {err:?}");
        }
    });
}

fn add_and_start_nodes(
    count: usize,
    mut nodes_to_start: Vec<String>,
    action_sender: UnboundedSender<Action>,
) {
    let peers = PeersArgs::default(); // will fetch from network contacts

    tokio::task::spawn_local(async move {
        let result = sn_node_manager::cmd::node::add(
            Some(count as u16),
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
            false,
            None,
            None,
            None,
            sn_node_manager::VerbosityLevel::Minimal,
        )
        .await;
        match result {
            Ok(added_services) => {
                info!("Successfully added service: {added_services:?}");
                nodes_to_start.extend(added_services);
            }
            Err(err) => {
                error!("Error while adding service {err:?}")
            }
        };

        if let Err(err) = sn_node_manager::cmd::node::start(
            NODE_START_INTERVAL as u64,
            vec![],
            nodes_to_start,
            sn_node_manager::VerbosityLevel::Minimal,
        )
        .await
        {
            error!("Error while starting services {err:?}");
        } else {
            info!("Successfully started services");
        }
        if let Err(err) = action_sender.send(Action::HomeActions(
            HomeActions::ServiceManagerOperationCompleted,
        )) {
            error!("Error while sending action: {err:?}");
        }
    });
}
