// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::footer::NodesToStart;
use super::header::SelectedMenuItem;
use super::{
    footer::Footer, header::Header, popup::manage_nodes::GB_PER_NODE, utils::centered_rect_fixed,
    Component, Frame,
};
use crate::action::OptionsActions;
use crate::config::get_launchpad_nodes_data_dir_path;
use crate::{
    action::{Action, StatusActions},
    config::Config,
    mode::{InputMode, Scene},
    node_stats::NodeStats,
    style::{
        clear_area, EUCALYPTUS, GHOST_WHITE, LIGHT_PERIWINKLE, VERY_LIGHT_AZURE, VIVID_SKY_BLUE,
    },
};
use color_eyre::eyre::{OptionExt, Result};
use crossterm::event::KeyEvent;
use ratatui::text::Span;
use ratatui::{prelude::*, widgets::*};
use sn_node_manager::config::get_node_registry_path;
use sn_peers_acquisition::PeersArgs;
use sn_service_management::{
    control::ServiceController, NodeRegistry, NodeServiceData, ServiceStatus,
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
    vec,
};
use tokio::sync::mpsc::UnboundedSender;

use super::super::node_mgmt::{maintain_n_running_nodes, reset_nodes, stop_nodes};

const NODE_STAT_UPDATE_INTERVAL: Duration = Duration::from_secs(5);
/// If nat detection fails for more than 3 times, we don't want to waste time running during every node start.
const MAX_ERRORS_WHILE_RUNNING_NAT_DETECTION: usize = 3;

#[derive(Clone)]
pub struct Status {
    /// Whether the component is active right now, capturing keystrokes + drawing things.
    active: bool,
    action_sender: Option<UnboundedSender<Action>>,
    config: Config,
    // state
    node_services: Vec<NodeServiceData>,
    is_nat_status_determined: bool,
    error_while_running_nat_detection: usize,
    node_stats: NodeStats,
    node_stats_last_update: Instant,
    node_table_state: TableState,
    nodes_to_start: usize,
    discord_username: String,
    // Currently the node registry file does not support concurrent actions and thus can lead to
    // inconsistent state. Another solution would be to have a file lock/db.
    lock_registry: Option<LockRegistryState>,
    // Peers to pass into nodes for startup
    peers_args: PeersArgs,
    // If path is provided, we don't fetch the binary from the network
    safenode_path: Option<PathBuf>,
    // Path where the node data is stored
    data_dir_path: PathBuf,
}

#[derive(Clone)]
pub enum LockRegistryState {
    StartingNodes,
    StoppingNodes,
    ResettingNodes,
}

impl Status {
    pub async fn new(
        allocated_disk_space: usize,
        discord_username: &str,
        peers_args: PeersArgs,
        safenode_path: Option<PathBuf>,
        data_dir_path: PathBuf,
    ) -> Result<Self> {
        let mut status = Self {
            peers_args,
            action_sender: Default::default(),
            config: Default::default(),
            active: true,
            node_services: Default::default(),
            is_nat_status_determined: false,
            error_while_running_nat_detection: 0,
            node_stats: NodeStats::default(),
            node_stats_last_update: Instant::now(),
            nodes_to_start: allocated_disk_space,
            node_table_state: Default::default(),
            lock_registry: None,
            discord_username: discord_username.to_string(),
            safenode_path,
            data_dir_path,
        };

        let now = Instant::now();
        debug!("Refreshing node registry states on startup");
        let mut node_registry = NodeRegistry::load(&get_node_registry_path()?)?;
        sn_node_manager::refresh_node_registry(
            &mut node_registry,
            &ServiceController {},
            false,
            true,
        )
        .await?;
        node_registry.save()?;
        debug!("Node registry states refreshed in {:?}", now.elapsed());
        status.load_node_registry_and_update_states()?;

        Ok(status)
    }

    /// Tries to trigger the update of node stats if the last update was more than `NODE_STAT_UPDATE_INTERVAL` ago.
    /// The result is sent via the StatusActions::NodesStatsObtained action.
    fn try_update_node_stats(&mut self, force_update: bool) -> Result<()> {
        if self.node_stats_last_update.elapsed() > NODE_STAT_UPDATE_INTERVAL || force_update {
            self.node_stats_last_update = Instant::now();

            NodeStats::fetch_all_node_stats(&self.node_services, self.get_actions_sender()?);
        }
        Ok(())
    }
    fn get_actions_sender(&self) -> Result<UnboundedSender<Action>> {
        self.action_sender
            .clone()
            .ok_or_eyre("Action sender not registered")
    }

    fn load_node_registry_and_update_states(&mut self) -> Result<()> {
        let node_registry = NodeRegistry::load(&get_node_registry_path()?)?;
        self.is_nat_status_determined = node_registry.nat_status.is_some();
        self.node_services = node_registry
            .nodes
            .into_iter()
            .filter(|node| node.status != ServiceStatus::Removed)
            .collect();
        info!(
            "Loaded node registry. Running nodes: {:?}",
            self.node_services.len()
        );

        if !self.node_services.is_empty() && self.node_table_state.selected().is_none() {
            self.node_table_state.select(Some(0));
        }

        Ok(())
    }

    /// Only run NAT detection if we haven't determined the status yet and we haven't failed more than 3 times.
    fn should_we_run_nat_detection(&self) -> bool {
        !self.is_nat_status_determined
            && self.error_while_running_nat_detection < MAX_ERRORS_WHILE_RUNNING_NAT_DETECTION
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

impl Component for Status {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.action_sender = Some(tx);

        // Update the stats to be shown as soon as the app is run
        self.try_update_node_stats(true)?;

        Ok(())
    }

    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        self.config = config;
        Ok(())
    }

    #[allow(clippy::comparison_chain)]
    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::Tick => {
                self.try_update_node_stats(false)?;
            }
            Action::SwitchScene(scene) => match scene {
                Scene::Status => {
                    self.active = true;
                    // make sure we're in navigation mode
                    return Ok(Some(Action::SwitchInputMode(InputMode::Navigation)));
                }
                Scene::ManageNodesPopUp => self.active = true,
                _ => self.active = false,
            },
            Action::StoreNodesToStart(count) => {
                self.nodes_to_start = count;
                if self.nodes_to_start == 0 {
                    info!("Nodes to start set to 0. Sending command to stop all nodes.");
                    return Ok(Some(Action::StatusActions(StatusActions::StopNodes)));
                } else {
                    info!("Nodes to start set to: {count}. Sending command to start nodes");
                    return Ok(Some(Action::StatusActions(StatusActions::StartNodes)));
                }
            }
            Action::StoreDiscordUserName(username) => {
                debug!("Storing discord username: {username:?}");
                let has_changed = self.discord_username != username;
                let we_have_nodes = !self.node_services.is_empty();

                self.discord_username = username;

                if we_have_nodes && has_changed {
                    self.lock_registry = Some(LockRegistryState::ResettingNodes);
                    info!("Resetting safenode services because the discord username was reset.");
                    let action_sender = self.get_actions_sender()?;
                    reset_nodes(action_sender, true);
                }
            }
            Action::StoreStorageDrive(ref drive_mountpoint, ref _drive_name) => {
                let action_sender = self.get_actions_sender()?;
                reset_nodes(action_sender, false);
                self.data_dir_path =
                    get_launchpad_nodes_data_dir_path(&drive_mountpoint.to_path_buf(), false)?;
            }
            Action::StatusActions(status_action) => {
                match status_action {
                    StatusActions::NodesStatsObtained(stats) => {
                        self.node_stats = stats;
                    }
                    StatusActions::StartNodesCompleted | StatusActions::StopNodesCompleted => {
                        self.lock_registry = None;
                        self.load_node_registry_and_update_states()?;
                    }
                    StatusActions::ResetNodesCompleted { trigger_start_node } => {
                        self.lock_registry = None;
                        self.load_node_registry_and_update_states()?;

                        if trigger_start_node {
                            debug!("Reset nodes completed. Triggering start nodes.");
                            return Ok(Some(Action::StatusActions(StatusActions::StartNodes)));
                        }
                        debug!("Reset nodes completed");
                    }
                    StatusActions::SuccessfullyDetectedNatStatus => {
                        debug!("Successfully detected nat status, is_nat_status_determined set to true");
                        self.is_nat_status_determined = true;
                    }
                    StatusActions::ErrorWhileRunningNatDetection => {
                        self.error_while_running_nat_detection += 1;
                        debug!(
                            "Error while running nat detection. Error count: {}",
                            self.error_while_running_nat_detection
                        );
                    }
                    StatusActions::TriggerManageNodes => {
                        return Ok(Some(Action::SwitchScene(Scene::ManageNodesPopUp)));
                    }
                    StatusActions::PreviousTableItem => {
                        self.select_previous_table_item();
                    }
                    StatusActions::NextTableItem => {
                        self.select_next_table_item();
                    }
                    StatusActions::StartNodes => {
                        debug!("Got action to start nodes");
                        if self.lock_registry.is_some() {
                            error!("Registry is locked. Cannot start node now.");
                            return Ok(None);
                        }

                        if self.nodes_to_start == 0 {
                            info!("Nodes to start not set. Ask for input.");
                            return Ok(Some(Action::StatusActions(
                                StatusActions::TriggerManageNodes,
                            )));
                        }

                        self.lock_registry = Some(LockRegistryState::StartingNodes);
                        let action_sender = self.get_actions_sender()?;
                        info!("Running maintain node count: {:?}", self.nodes_to_start);

                        maintain_n_running_nodes(
                            self.nodes_to_start as u16,
                            self.discord_username.clone(),
                            self.peers_args.clone(),
                            self.should_we_run_nat_detection(),
                            self.safenode_path.clone(),
                            Some(self.data_dir_path.clone()),
                            action_sender,
                        );
                    }
                    StatusActions::StopNodes => {
                        debug!("Got action to stop nodes");
                        if self.lock_registry.is_some() {
                            error!("Registry is locked. Cannot stop node now.");
                            return Ok(None);
                        }

                        let running_nodes = self.get_running_nodes();
                        self.lock_registry = Some(LockRegistryState::StoppingNodes);
                        let action_sender = self.get_actions_sender()?;
                        info!("Stopping node service: {running_nodes:?}");

                        stop_nodes(running_nodes, action_sender);
                    }
                }
            }
            Action::OptionsActions(OptionsActions::ResetNodes) => {
                debug!("Got action to reset nodes");
                if self.lock_registry.is_some() {
                    error!("Registry is locked. Cannot reset nodes now.");
                    return Ok(None);
                }

                self.lock_registry = Some(LockRegistryState::ResettingNodes);
                let action_sender = self.get_actions_sender()?;
                info!("Got action to reset nodes");
                reset_nodes(action_sender, false);
            }
            _ => {}
        }
        Ok(None)
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        if !self.active {
            return Ok(());
        }

        let layout = Layout::new(
            Direction::Vertical,
            [
                // Header
                Constraint::Length(1),
                // Device status
                Constraint::Max(6),
                // Node status
                Constraint::Min(3),
                // Footer
                Constraint::Length(3),
            ],
        )
        .split(area);

        // ==== Header =====

        let header = Header::new();
        f.render_stateful_widget(header, layout[0], &mut SelectedMenuItem::Status);

        // ==== Device Status =====

        if self.discord_username.is_empty() {
            let line1 = Line::from(vec![Span::styled(
                "Add this device to the Beta Rewards Program",
                Style::default().fg(VERY_LIGHT_AZURE),
            )]);
            let line2 = Line::from(vec![
                Span::styled("Press ", Style::default().fg(VERY_LIGHT_AZURE)),
                Span::styled("[Ctrl+B]", Style::default().fg(GHOST_WHITE)),
                Span::styled(" to add your ", Style::default().fg(VERY_LIGHT_AZURE)),
                Span::styled(
                    "Discord Username",
                    Style::default().fg(VERY_LIGHT_AZURE).bold(),
                ),
            ]);
            f.render_widget(
                Paragraph::new(vec![Line::raw(""), Line::raw(""), line1, line2]).block(
                    Block::default()
                        .title(" Device Status ")
                        .title_style(Style::new().fg(GHOST_WHITE))
                        .borders(Borders::ALL)
                        .padding(Padding::horizontal(1))
                        .border_style(Style::new().fg(VERY_LIGHT_AZURE)),
                ),
                layout[1],
            );
        } else {
            // Device Status as a table

            let storage_allocated_row = Row::new(vec![
                Cell::new("Storage Allocated".to_string()).fg(GHOST_WHITE),
                Cell::new(format!("{} GB", self.nodes_to_start * GB_PER_NODE)).fg(GHOST_WHITE),
            ]);
            let memory_use_val = if self.node_stats.memory_usage_mb as f64 / 1024_f64 > 1.0 {
                format!(
                    "{:.2} GB",
                    self.node_stats.memory_usage_mb as f64 / 1024_f64
                )
            } else {
                format!("{} MB", self.node_stats.memory_usage_mb)
            };

            let memory_use_row = Row::new(vec![
                Cell::new("Memory Use".to_string()).fg(GHOST_WHITE),
                Cell::new(memory_use_val).fg(GHOST_WHITE),
            ]);

            // Combine "Nanos Earned" and "Discord Username" into a single row
            let discord_username_title = Span::styled(
                "Discord Username: ".to_string(),
                Style::default().fg(VIVID_SKY_BLUE),
            );

            let discord_username = if !self.discord_username.is_empty() {
                Span::styled(
                    self.discord_username.clone(),
                    Style::default().fg(VIVID_SKY_BLUE),
                )
                .bold()
            } else {
                Span::styled(
                    "[Ctrl+B] to set".to_string(),
                    Style::default().fg(GHOST_WHITE),
                )
            };

            let total_nanos_earned_and_discord = Row::new(vec![
                Cell::new("Nanos Earned".to_string()).fg(VIVID_SKY_BLUE),
                Cell::new(self.node_stats.forwarded_rewards.to_string())
                    .fg(VIVID_SKY_BLUE)
                    .bold(),
                Cell::new(
                    Line::from(vec![discord_username_title, discord_username])
                        .alignment(Alignment::Right),
                ),
            ]);

            let stats_rows = vec![
                storage_allocated_row,
                memory_use_row.bottom_margin(1),
                total_nanos_earned_and_discord,
            ];
            let stats_width = [Constraint::Length(5)];
            let column_constraints = [
                Constraint::Percentage(25),
                Constraint::Percentage(5),
                Constraint::Percentage(70),
            ];
            let stats_table = Table::new(stats_rows, stats_width)
                .block(
                    Block::default()
                        .title(" Device Status ")
                        .title_style(Style::default().fg(GHOST_WHITE))
                        .borders(Borders::ALL)
                        .padding(Padding::horizontal(1))
                        .style(Style::default().fg(VERY_LIGHT_AZURE)),
                )
                .widths(column_constraints);
            f.render_widget(stats_table, layout[1]);
        };

        // ==== Node Status =====

        let node_rows: Vec<_> = self
            .node_services
            .iter()
            .filter_map(|n| {
                let peer_id = n.peer_id;
                if n.status == ServiceStatus::Removed {
                    return None;
                }
                let peer_id = peer_id.map(|p| p.to_string()).unwrap_or("-".to_string());
                let status = format!("{:?}", n.status);
                let version = format!("v{}", n.version);

                let row = vec![n.service_name.clone(), peer_id, version, status];
                let row_style = if n.status == ServiceStatus::Running {
                    Style::default().fg(EUCALYPTUS)
                } else {
                    Style::default().fg(GHOST_WHITE)
                };
                Some(Row::new(row).style(row_style))
            })
            .collect();

        if node_rows.is_empty() {
            let line1 = Line::from(vec![
                Span::styled("Press ", Style::default().fg(LIGHT_PERIWINKLE)),
                Span::styled("[Ctrl+G] ", Style::default().fg(GHOST_WHITE)),
                Span::styled("to Add and ", Style::default().fg(LIGHT_PERIWINKLE)),
                Span::styled("Start Nodes ", Style::default().fg(GHOST_WHITE)),
                Span::styled("on this device", Style::default().fg(LIGHT_PERIWINKLE)),
            ]);

            let line2 = Line::from(vec![Span::styled(
                "Each node will use 5GB of storage and a small amount of memory, \
                CPU, and Network bandwidth. Most computers can run many nodes at once, \
                but we recommend you add them gradually",
                Style::default().fg(LIGHT_PERIWINKLE),
            )]);

            f.render_widget(
                Paragraph::new(vec![Line::raw(""), line1, Line::raw(""), line2])
                    .wrap(Wrap { trim: false })
                    .fg(LIGHT_PERIWINKLE)
                    .block(
                        Block::default()
                            .title(" Nodes (0) ".to_string())
                            .title_style(Style::default().fg(LIGHT_PERIWINKLE))
                            .borders(Borders::ALL)
                            .border_style(style::Style::default().fg(EUCALYPTUS))
                            .padding(Padding::horizontal(1)),
                    ),
                layout[2],
            );
        } else {
            let node_widths = [
                Constraint::Max(15),
                Constraint::Min(40),
                Constraint::Max(10),
                Constraint::Max(10),
            ];
            let table = Table::new(node_rows.clone(), node_widths)
                .column_spacing(2)
                .highlight_style(Style::new().reversed())
                .block(
                    Block::default()
                        .title(format!(" Nodes ({}) ", self.nodes_to_start))
                        .padding(Padding::new(2, 2, 1, 1))
                        .title_style(Style::default().fg(GHOST_WHITE))
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(EUCALYPTUS)),
                )
                .highlight_symbol("*");
            f.render_stateful_widget(table, layout[2], &mut self.node_table_state);
        }

        // ==== Footer =====

        let footer = Footer::default();
        let footer_state = if !node_rows.is_empty() {
            &mut NodesToStart::Configured
        } else {
            &mut NodesToStart::NotConfigured
        };
        f.render_stateful_widget(footer, layout[3], footer_state);

        // ===== Popup =====

        if let Some(registry_state) = &self.lock_registry {
            let popup_area = centered_rect_fixed(50, 12, area);
            clear_area(f, popup_area);

            let popup_border = Paragraph::new("").block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Manage Nodes ")
                    .title_style(Style::new().fg(VIVID_SKY_BLUE))
                    .padding(Padding::uniform(2))
                    .border_style(Style::new().fg(GHOST_WHITE)),
            );

            let popup_text = match registry_state {
                LockRegistryState::StartingNodes => {
                    if self.should_we_run_nat_detection() {
                        vec![
                            Line::raw("Starting nodes..."),
                            Line::raw(""),
                            Line::raw(""),
                            Line::raw("Please wait, performing initial NAT detection"),
                            Line::raw("This may take a couple minutes."),
                        ]
                    } else {
                        vec![Line::raw("Starting nodes...")]
                    }
                }
                LockRegistryState::StoppingNodes => vec![Line::raw("Stopping nodes...")],
                LockRegistryState::ResettingNodes => vec![Line::raw("Resetting nodes...")],
            };
            let centred_area = Layout::new(
                Direction::Vertical,
                vec![
                    // border
                    Constraint::Length(1),
                    Constraint::Min(1),
                    // our text goes here
                    Constraint::Length(3),
                    Constraint::Min(1),
                    // border
                    Constraint::Length(1),
                ],
            )
            .split(popup_area)[2];
            let text = Paragraph::new(popup_text)
                .block(Block::default().padding(Padding::horizontal(2)))
                .wrap(Wrap { trim: false })
                .alignment(Alignment::Center)
                .fg(EUCALYPTUS);
            f.render_widget(text, centred_area);

            f.render_widget(popup_border, popup_area);
        }

        Ok(())
    }

    fn handle_key_events(&mut self, key: KeyEvent) -> Result<Vec<Action>> {
        debug!("Key received in Status: {:?}", key);
        Ok(vec![])
    }
}
