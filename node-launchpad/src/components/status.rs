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
use crate::components::popup::port_range::PORT_ALLOCATION;
use crate::config::get_launchpad_nodes_data_dir_path;
use crate::connection_mode::ConnectionMode;
use crate::error::ErrorPopup;
use crate::node_mgmt::MaintainNodesArgs;
use crate::node_mgmt::{PORT_MAX, PORT_MIN};
use crate::style::{COOL_GREY, INDIGO};
use crate::tui::Event;
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
use sn_node_manager::add_services::config::PortRange;
use sn_node_manager::config::get_node_registry_path;
use sn_peers_acquisition::PeersArgs;
use sn_service_management::{
    control::ServiceController, NodeRegistry, NodeServiceData, ServiceStatus,
};
use std::collections::HashMap;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
    vec,
};
use tokio::sync::mpsc::UnboundedSender;

use super::super::node_mgmt::{maintain_n_running_nodes, reset_nodes, stop_nodes};

use throbber_widgets_tui::{self, ThrobberState};

pub const NODE_STAT_UPDATE_INTERVAL: Duration = Duration::from_secs(5);
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
    node_services_throttle_state: HashMap<String, ThrobberState>,
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
    // Connection mode
    connection_mode: ConnectionMode,
    // Port from
    port_from: Option<u32>,
    // Port to
    port_to: Option<u32>,
    error_popup: Option<ErrorPopup>,
}

#[derive(Clone)]
pub enum LockRegistryState {
    StartingNodes,
    StoppingNodes,
    ResettingNodes,
}

pub struct StatusConfig {
    pub allocated_disk_space: usize,
    pub discord_username: String,
    pub peers_args: PeersArgs,
    pub safenode_path: Option<PathBuf>,
    pub data_dir_path: PathBuf,
    pub connection_mode: ConnectionMode,
    pub port_from: Option<u32>,
    pub port_to: Option<u32>,
}

impl Status {
    pub async fn new(config: StatusConfig) -> Result<Self> {
        let mut status = Self {
            peers_args: config.peers_args,
            action_sender: Default::default(),
            config: Default::default(),
            active: true,
            node_services: Default::default(),
            node_services_throttle_state: HashMap::new(),
            is_nat_status_determined: false,
            error_while_running_nat_detection: 0,
            node_stats: NodeStats::default(),
            node_stats_last_update: Instant::now(),
            nodes_to_start: config.allocated_disk_space,
            node_table_state: Default::default(),
            lock_registry: None,
            discord_username: config.discord_username,
            safenode_path: config.safenode_path,
            data_dir_path: config.data_dir_path,
            connection_mode: config.connection_mode,
            port_from: config.port_from,
            port_to: config.port_to,
            error_popup: None,
        };

        let now = Instant::now();
        debug!("Refreshing node registry states on startup");
        let mut node_registry = NodeRegistry::load(&get_node_registry_path()?)?;
        sn_node_manager::refresh_node_registry(
            &mut node_registry,
            &ServiceController {},
            false,
            true,
            false,
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
            "Loaded node registry. Maintaining {:?} nodes.",
            self.node_services.len()
        );

        if !self.node_services.is_empty() && self.node_table_state.selected().is_none() {
            // self.node_table_state.select(Some(0));
            self.node_table_state.select(None);
        }

        Ok(())
    }

    /// Only run NAT detection if we haven't determined the status yet and we haven't failed more than 3 times.
    fn should_we_run_nat_detection(&self) -> bool {
        self.connection_mode == ConnectionMode::Automatic
            && !self.is_nat_status_determined
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

    fn _select_next_table_item(&mut self) {
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

    fn _select_previous_table_item(&mut self) {
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

    fn handle_events(&mut self, event: Option<Event>) -> Result<Vec<Action>> {
        let r = match event {
            Some(Event::Key(key_event)) => self.handle_key_events(key_event)?,
            _ => vec![],
        };
        Ok(r)
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::Tick => {
                self.try_update_node_stats(false)?;
                for (_spinner_key, spinner_state) in self.node_services_throttle_state.iter_mut() {
                    spinner_state.calc_next(); // Assuming calc_next() is a method of ThrobberState
                }
            }
            Action::SwitchScene(scene) => match scene {
                Scene::Status | Scene::StatusBetaProgrammePopUp => {
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
                    debug!("Setting lock_registry to ResettingNodes");
                    self.lock_registry = Some(LockRegistryState::ResettingNodes);
                    info!("Resetting safenode services because the Discord Username was reset.");
                    let action_sender = self.get_actions_sender()?;
                    reset_nodes(action_sender, true);
                }
            }
            Action::StoreStorageDrive(ref drive_mountpoint, ref _drive_name) => {
                debug!("Setting lock_registry to ResettingNodes");
                self.lock_registry = Some(LockRegistryState::ResettingNodes);
                info!("Resetting safenode services because the Storage Drive was changed.");
                let action_sender = self.get_actions_sender()?;
                reset_nodes(action_sender, false);
                self.data_dir_path =
                    get_launchpad_nodes_data_dir_path(&drive_mountpoint.to_path_buf(), false)?;
            }
            Action::StoreConnectionMode(connection_mode) => {
                debug!("Setting lock_registry to ResettingNodes");
                self.lock_registry = Some(LockRegistryState::ResettingNodes);
                self.connection_mode = connection_mode;
                info!("Resetting safenode services because the Connection Mode range was changed.");
                let action_sender = self.get_actions_sender()?;
                reset_nodes(action_sender, false);
            }
            Action::StorePortRange(port_from, port_range) => {
                debug!("Setting lock_registry to ResettingNodes");
                self.lock_registry = Some(LockRegistryState::ResettingNodes);
                self.port_from = Some(port_from);
                self.port_to = Some(port_range);
                info!("Resetting safenode services because the Port Range was changed.");
                let action_sender = self.get_actions_sender()?;
                reset_nodes(action_sender, false);
            }
            Action::StatusActions(status_action) => match status_action {
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
                    debug!(
                        "Successfully detected nat status, is_nat_status_determined set to true"
                    );
                    self.is_nat_status_determined = true;
                }
                StatusActions::ErrorWhileRunningNatDetection => {
                    self.error_while_running_nat_detection += 1;
                    debug!(
                        "Error while running nat detection. Error count: {}",
                        self.error_while_running_nat_detection
                    );
                }
                StatusActions::ErrorLoadingNodeRegistry { raw_error }
                | StatusActions::ErrorGettingNodeRegistryPath { raw_error } => {
                    self.error_popup = Some(ErrorPopup::new(
                        "Error".to_string(),
                        "Error getting node registry path".to_string(),
                        raw_error,
                    ));
                    if let Some(error_popup) = &mut self.error_popup {
                        error_popup.show();
                    }
                    // Switch back to entry mode so we can handle key events
                    return Ok(Some(Action::SwitchInputMode(InputMode::Entry)));
                }
                StatusActions::ErrorScalingUpNodes { raw_error } => {
                    self.error_popup = Some(ErrorPopup::new(
                        "Error".to_string(),
                        "Error adding new nodes".to_string(),
                        raw_error,
                    ));
                    if let Some(error_popup) = &mut self.error_popup {
                        error_popup.show();
                    }
                    // Switch back to entry mode so we can handle key events
                    return Ok(Some(Action::SwitchInputMode(InputMode::Entry)));
                }
                StatusActions::ErrorStoppingNodes { raw_error } => {
                    self.error_popup = Some(ErrorPopup::new(
                        "Error".to_string(),
                        "Error stopping nodes".to_string(),
                        raw_error,
                    ));
                    if let Some(error_popup) = &mut self.error_popup {
                        error_popup.show();
                    }
                    // Switch back to entry mode so we can handle key events
                    return Ok(Some(Action::SwitchInputMode(InputMode::Entry)));
                }
                StatusActions::ErrorResettingNodes { raw_error } => {
                    self.error_popup = Some(ErrorPopup::new(
                        "Error".to_string(),
                        "Error resetting nodes".to_string(),
                        raw_error,
                    ));
                    if let Some(error_popup) = &mut self.error_popup {
                        error_popup.show();
                    }
                    // Switch back to entry mode so we can handle key events
                    return Ok(Some(Action::SwitchInputMode(InputMode::Entry)));
                }
                StatusActions::TriggerManageNodes => {
                    return Ok(Some(Action::SwitchScene(Scene::ManageNodesPopUp)));
                }
                StatusActions::PreviousTableItem => {
                    // self.select_previous_table_item();
                }
                StatusActions::NextTableItem => {
                    // self.select_next_table_item();
                }
                StatusActions::StartNodes => {
                    debug!("Got action to start nodes");

                    if self.nodes_to_start == 0 {
                        info!("Nodes to start not set. Ask for input.");
                        return Ok(Some(Action::StatusActions(
                            StatusActions::TriggerManageNodes,
                        )));
                    }

                    if self.lock_registry.is_some() {
                        error!("Registry is locked. Cannot start node now.");
                        return Ok(None);
                    }

                    debug!("Setting lock_registry to StartingNodes");
                    self.lock_registry = Some(LockRegistryState::StartingNodes);

                    let port_range = PortRange::Range(
                        self.port_from.unwrap_or(PORT_MIN) as u16,
                        self.port_to.unwrap_or(PORT_MAX) as u16,
                    );

                    let action_sender = self.get_actions_sender()?;

                    let maintain_nodes_args = MaintainNodesArgs {
                        count: self.nodes_to_start as u16,
                        owner: self.discord_username.clone(),
                        peers_args: self.peers_args.clone(),
                        run_nat_detection: self.should_we_run_nat_detection(),
                        safenode_path: self.safenode_path.clone(),
                        data_dir_path: Some(self.data_dir_path.clone()),
                        action_sender: action_sender.clone(),
                        connection_mode: self.connection_mode,
                        port_range: Some(port_range),
                    };

                    debug!("Calling maintain_n_running_nodes");

                    maintain_n_running_nodes(maintain_nodes_args);
                }
                StatusActions::StopNodes => {
                    debug!("Got action to stop nodes");
                    if self.lock_registry.is_some() {
                        error!("Registry is locked. Cannot stop node now.");
                        return Ok(None);
                    }

                    let running_nodes = self.get_running_nodes();
                    debug!("Setting lock_registry to StoppingNodes");
                    self.lock_registry = Some(LockRegistryState::StoppingNodes);
                    let action_sender = self.get_actions_sender()?;
                    info!("Stopping node service: {running_nodes:?}");

                    stop_nodes(running_nodes, action_sender);
                }
                StatusActions::TriggerBetaProgramme => {
                    return Ok(Some(Action::SwitchScene(Scene::StatusBetaProgrammePopUp)));
                }
            },
            Action::OptionsActions(OptionsActions::ResetNodes) => {
                debug!("Got action to reset nodes");
                if self.lock_registry.is_some() {
                    error!("Registry is locked. Cannot reset nodes now.");
                    return Ok(None);
                }

                debug!("Setting lock_registry to ResettingNodes");
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

        // Device Status as a block with two tables so we can shrink the screen
        // and preserve as much as we can information

        let combined_block = Block::default()
            .title(" Device Status ")
            .bold()
            .title_style(Style::default().fg(GHOST_WHITE))
            .borders(Borders::ALL)
            .padding(Padding::horizontal(1))
            .style(Style::default().fg(VERY_LIGHT_AZURE));

        f.render_widget(combined_block.clone(), layout[1]);

        let storage_allocated_row = Row::new(vec![
            Cell::new("Storage Allocated".to_string()).fg(GHOST_WHITE),
            Cell::new(format!("{} GB", self.nodes_to_start * GB_PER_NODE)).fg(GHOST_WHITE),
        ]);
        let memory_use_val = if self.node_stats.total_memory_usage_mb as f64 / 1024_f64 > 1.0 {
            format!(
                "{:.2} GB",
                self.node_stats.total_memory_usage_mb as f64 / 1024_f64
            )
        } else {
            format!("{} MB", self.node_stats.total_memory_usage_mb)
        };

        let memory_use_row = Row::new(vec![
            Cell::new("Memory Use".to_string()).fg(GHOST_WHITE),
            Cell::new(memory_use_val).fg(GHOST_WHITE),
        ]);

        let connection_mode_string = match self.connection_mode {
            ConnectionMode::HomeNetwork => "Home Network",
            ConnectionMode::UPnP => "UPnP",
            ConnectionMode::CustomPorts => &format!(
                "Custom Ports  {}-{}",
                self.port_from.unwrap_or(PORT_MIN),
                self.port_to.unwrap_or(PORT_MIN + PORT_ALLOCATION)
            ),
            ConnectionMode::Automatic => "Automatic",
        };

        let connection_mode_row = Row::new(vec![
            Cell::new("Connection".to_string()).fg(GHOST_WHITE),
            Cell::new(connection_mode_string).fg(LIGHT_PERIWINKLE),
        ]);

        let stats_rows = vec![storage_allocated_row, memory_use_row, connection_mode_row];
        let stats_width = [Constraint::Length(5)];
        let column_constraints = [Constraint::Length(23), Constraint::Fill(1)];
        let stats_table = Table::new(stats_rows, stats_width).widths(column_constraints);

        // Combine "Nanos Earned" and "Username" into a single row
        let discord_username_placeholder = "Username: "; // Used to calculate the width of the username column
        let discord_username_no_username = "[Ctrl+B] to set";
        let discord_username_title = Span::styled(
            discord_username_placeholder,
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
                discord_username_no_username,
                Style::default().fg(GHOST_WHITE),
            )
        };

        let total_nanos_earned_and_discord_row = Row::new(vec![
            Cell::new("Nanos Earned".to_string()).fg(VIVID_SKY_BLUE),
            Cell::new(self.node_stats.total_forwarded_rewards.to_string())
                .fg(VIVID_SKY_BLUE)
                .bold(),
            Cell::new(
                Line::from(vec![discord_username_title, discord_username])
                    .alignment(Alignment::Right),
            ),
        ]);

        let nanos_discord_rows = vec![total_nanos_earned_and_discord_row];
        let nanos_discord_width = [Constraint::Length(5)];
        let column_constraints = [
            Constraint::Length(23),
            Constraint::Fill(1),
            Constraint::Length(
                discord_username_placeholder.len() as u16
                    + if !self.discord_username.is_empty() {
                        self.discord_username.len() as u16
                    } else {
                        discord_username_no_username.len() as u16
                    },
            ),
        ];
        let nanos_discord_table =
            Table::new(nanos_discord_rows, nanos_discord_width).widths(column_constraints);

        let inner_area = combined_block.inner(layout[1]);
        let device_layout = Layout::new(
            Direction::Vertical,
            vec![Constraint::Length(5), Constraint::Length(1)],
        )
        .split(inner_area);

        // Render both tables inside the combined block
        f.render_widget(stats_table, device_layout[0]);
        f.render_widget(nanos_discord_table, device_layout[1]);

        // ==== Node Status =====

        // Widths
        const NODE_WIDTH: usize = 10;
        const VERSION_WIDTH: usize = 7;
        const NANOS_WIDTH: usize = 5;
        const MEMORY_WIDTH: usize = 7;
        const MBPS_WIDTH: usize = 13;
        const RECORDS_WIDTH: usize = 4;
        const PEERS_WIDTH: usize = 5;
        const CONNS_WIDTH: usize = 5;
        const STATUS_WIDTH: usize = 8;
        const SPINNER_WIDTH: usize = 1;

        let node_rows: Vec<_> = self
            .node_services
            .iter()
            .filter_map(|n| {
                if n.status == ServiceStatus::Removed {
                    return None;
                }

                let mut status = format!("{:?}", n.status);
                if let Some(LockRegistryState::StartingNodes) = self.lock_registry {
                    status = "Starting".to_string();
                }
                let connected_peers = match n.connected_peers {
                    Some(ref peers) => format!("{:?}", peers.len()),
                    None => "0".to_string(),
                };

                let mut nanos = "-".to_string();
                let mut memory = "-".to_string();
                let mut mbps = "        -".to_string();
                let mut records = "-".to_string();
                let mut connections = "-".to_string();

                let individual_stats = self
                    .node_stats
                    .individual_stats
                    .iter()
                    .find(|s| s.service_name == n.service_name);
                if let Some(stats) = individual_stats {
                    nanos = stats.forwarded_rewards.to_string();
                    memory = stats.memory_usage_mb.to_string();
                    mbps = format!(
                        "↓{:05.2} ↑{:05.2}",
                        stats.bandwidth_inbound as f64 / (1024_f64 * 1024_f64),
                        stats.bandwidth_outbound as f64 / (1024_f64 * 1024_f64)
                    );
                    records = stats.max_records.to_string();
                    connections = stats.connections.to_string();
                }

                // Create a row vector
                let row = vec![
                    n.service_name.clone().to_string(),
                    n.version.to_string(),
                    format!(
                        "{}{}",
                        " ".repeat(NANOS_WIDTH.saturating_sub(nanos.len())),
                        nanos.to_string()
                    ),
                    format!(
                        "{}{} MB",
                        " ".repeat(MEMORY_WIDTH.saturating_sub(memory.len() + 4)),
                        memory.to_string()
                    ),
                    mbps.to_string(),
                    format!(
                        "{}{}",
                        " ".repeat(RECORDS_WIDTH.saturating_sub(records.len())),
                        records.to_string()
                    ),
                    format!(
                        "{}{}",
                        " ".repeat(PEERS_WIDTH.saturating_sub(connected_peers.len())),
                        connected_peers.to_string()
                    ),
                    format!(
                        "{}{}",
                        " ".repeat(CONNS_WIDTH.saturating_sub(connections.len())),
                        connections.to_string()
                    ),
                    status.to_string(),
                ];

                // Create a styled row
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
                Span::styled("[Ctrl+G] ", Style::default().fg(GHOST_WHITE).bold()),
                Span::styled("to Add and ", Style::default().fg(LIGHT_PERIWINKLE)),
                Span::styled("Start Nodes ", Style::default().fg(GHOST_WHITE).bold()),
                Span::styled("on this device", Style::default().fg(LIGHT_PERIWINKLE)),
            ]);

            let line2 = Line::from(vec![Span::styled(
                format!(
                    "Each node will use {}GB of storage and a small amount of memory, \
                CPU, and Network bandwidth. Most computers can run many nodes at once, \
                but we recommend you add them gradually",
                    GB_PER_NODE
                ),
                Style::default().fg(LIGHT_PERIWINKLE),
            )]);

            f.render_widget(
                Paragraph::new(vec![Line::raw(""), line1, Line::raw(""), line2])
                    .wrap(Wrap { trim: false })
                    .fg(LIGHT_PERIWINKLE)
                    .block(
                        Block::default()
                            .title(Line::from(vec![
                                Span::styled(" Nodes", Style::default().fg(GHOST_WHITE).bold()),
                                Span::styled(" (0) ", Style::default().fg(LIGHT_PERIWINKLE)),
                            ]))
                            .title_style(Style::default().fg(LIGHT_PERIWINKLE))
                            .borders(Borders::ALL)
                            .border_style(style::Style::default().fg(EUCALYPTUS))
                            .padding(Padding::horizontal(1)),
                    ),
                layout[2],
            );
        } else {
            // Node/s block
            let block_nodes = Block::default()
                .title(Line::from(vec![
                    Span::styled(" Nodes", Style::default().fg(GHOST_WHITE).bold()),
                    Span::styled(
                        format!(" ({}) ", self.nodes_to_start),
                        Style::default().fg(LIGHT_PERIWINKLE),
                    ),
                ]))
                .padding(Padding::new(1, 1, 0, 0))
                .title_style(Style::default().fg(GHOST_WHITE))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(EUCALYPTUS));

            // Create a layout to arrange the header and table vertically
            let inner_layout = Layout::new(
                Direction::Vertical,
                vec![Constraint::Length(1), Constraint::Min(0)],
            );

            // Split the inner area of the combined block
            let inner_area = block_nodes.inner(layout[2]);
            let inner_chunks = inner_layout.split(inner_area);

            // Column Widths
            let node_widths = [
                Constraint::Min(NODE_WIDTH as u16),
                Constraint::Min(VERSION_WIDTH as u16),
                Constraint::Min(NANOS_WIDTH as u16),
                Constraint::Min(MEMORY_WIDTH as u16),
                Constraint::Min(MBPS_WIDTH as u16),
                Constraint::Min(RECORDS_WIDTH as u16),
                Constraint::Min(PEERS_WIDTH as u16),
                Constraint::Min(CONNS_WIDTH as u16),
                Constraint::Min(STATUS_WIDTH as u16),
                Constraint::Max(SPINNER_WIDTH as u16),
            ];

            // Header
            let header_row = Row::new(vec![
                Cell::new("Node").fg(COOL_GREY),
                Cell::new("Version").fg(COOL_GREY),
                Cell::new("Nanos").fg(COOL_GREY),
                Cell::new("Memory").fg(COOL_GREY),
                Cell::new(
                    format!("{}{}", " ".repeat(MBPS_WIDTH - "Mbps".len()), "Mbps").fg(COOL_GREY),
                ),
                Cell::new("Recs").fg(COOL_GREY),
                Cell::new("Peers").fg(COOL_GREY),
                Cell::new("Conns").fg(COOL_GREY),
                Cell::new("Status").fg(COOL_GREY),
                Cell::new(" ").fg(COOL_GREY), // Spinner
            ]);

            let header = Table::new(vec![header_row.clone()], node_widths)
                .style(Style::default().add_modifier(Modifier::BOLD));

            // Table items
            let table = Table::new(node_rows.clone(), node_widths)
                .column_spacing(1)
                .highlight_style(Style::default().bg(INDIGO))
                .highlight_spacing(HighlightSpacing::Always);

            f.render_stateful_widget(header, inner_chunks[0], &mut self.node_table_state);
            f.render_stateful_widget(table, inner_chunks[1], &mut self.node_table_state);

            // Render the throbber in the last column for running nodes
            for (i, node) in self.node_services.iter().enumerate() {
                let mut throbber = throbber_widgets_tui::Throbber::default()
                    .throbber_set(throbber_widgets_tui::BRAILLE_SIX_DOUBLE);
                match node.status {
                    ServiceStatus::Running => {
                        throbber = throbber
                            .throbber_style(
                                Style::default().fg(EUCALYPTUS).add_modifier(Modifier::BOLD),
                            )
                            .use_type(throbber_widgets_tui::WhichUse::Spin);
                    }
                    ServiceStatus::Stopped => {
                        throbber = throbber
                            .throbber_style(
                                Style::default()
                                    .fg(GHOST_WHITE)
                                    .add_modifier(Modifier::BOLD),
                            )
                            .use_type(throbber_widgets_tui::WhichUse::Full);
                    }
                    _ => {}
                }
                if let Some(LockRegistryState::StartingNodes) = self.lock_registry {
                    throbber = throbber
                        .throbber_style(
                            Style::default().fg(EUCALYPTUS).add_modifier(Modifier::BOLD),
                        )
                        .throbber_set(throbber_widgets_tui::BOX_DRAWING)
                        .use_type(throbber_widgets_tui::WhichUse::Spin);
                }
                let throbber_area =
                    Rect::new(inner_chunks[1].width, inner_chunks[1].y + i as u16, 1, 1);
                let throttle_state = self
                    .node_services_throttle_state
                    .entry(node.service_name.clone())
                    .or_default();
                f.render_stateful_widget(throbber, throbber_area, throttle_state);
            }
            f.render_widget(block_nodes, layout[2]);
        }

        // ==== Footer =====

        let footer = Footer::default();
        let footer_state = if !node_rows.is_empty() {
            if !self.get_running_nodes().is_empty() {
                &mut NodesToStart::Running
            } else {
                &mut NodesToStart::Configured
            }
        } else {
            &mut NodesToStart::NotConfigured
        };
        f.render_stateful_widget(footer, layout[3], footer_state);

        // ===== Popup =====

        // Error Popup
        if let Some(error_popup) = &self.error_popup {
            if error_popup.is_visible() {
                error_popup.draw_error(f, area);

                return Ok(());
            }
        }

        // Status Popup
        if let Some(registry_state) = &self.lock_registry {
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
                        // We avoid rendering the popup as we have status lines now
                        return Ok(());
                    }
                }
                LockRegistryState::StoppingNodes => {
                    vec![
                        Line::raw(""),
                        Line::raw(""),
                        Line::raw(""),
                        Line::raw("Stopping nodes..."),
                    ]
                }
                LockRegistryState::ResettingNodes => {
                    vec![
                        Line::raw(""),
                        Line::raw(""),
                        Line::raw(""),
                        Line::raw("Resetting nodes..."),
                    ]
                }
            };
            if !popup_text.is_empty() {
                let popup_area = centered_rect_fixed(50, 12, area);
                clear_area(f, popup_area);

                let popup_border = Paragraph::new("").block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Manage Nodes ")
                        .bold()
                        .title_style(Style::new().fg(VIVID_SKY_BLUE))
                        .padding(Padding::uniform(2))
                        .border_style(Style::new().fg(GHOST_WHITE)),
                );

                let centred_area = Layout::new(
                    Direction::Vertical,
                    vec![
                        // border
                        Constraint::Length(2),
                        // our text goes here
                        Constraint::Min(5),
                        // border
                        Constraint::Length(1),
                    ],
                )
                .split(popup_area);
                let text = Paragraph::new(popup_text)
                    .block(Block::default().padding(Padding::horizontal(2)))
                    .wrap(Wrap { trim: false })
                    .alignment(Alignment::Center)
                    .fg(EUCALYPTUS);
                f.render_widget(text, centred_area[1]);

                f.render_widget(popup_border, popup_area);
            }
        }

        Ok(())
    }

    fn handle_key_events(&mut self, key: KeyEvent) -> Result<Vec<Action>> {
        debug!("Key received in Status: {:?}", key);
        if let Some(error_popup) = &mut self.error_popup {
            if error_popup.is_visible() {
                error_popup.handle_input(key);
                return Ok(vec![Action::SwitchInputMode(InputMode::Navigation)]);
            }
        }
        Ok(vec![])
    }
}
