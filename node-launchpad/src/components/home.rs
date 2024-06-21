// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{manage_nodes::GB_PER_NODE, utils::centered_rect_fixed, Component, Frame};
use crate::{
    action::{Action, HomeActions},
    config::Config,
    mode::{InputMode, Scene},
    node_stats::NodeStats,
    style::{
        clear_area, COOL_GREY, EUCALYPTUS, GHOST_WHITE, LIGHT_PERIWINKLE, VERY_LIGHT_AZURE,
        VIVID_SKY_BLUE,
    },
};
use color_eyre::eyre::{OptionExt, Result};
use rand::seq::SliceRandom;
use ratatui::{prelude::*, widgets::*};
use sn_node_manager::{config::get_node_registry_path, VerbosityLevel};
use sn_peers_acquisition::{get_bootstrap_peers_from_url, PeersArgs};
use sn_service_management::{
    control::ServiceController, NodeRegistry, NodeServiceData, ServiceStatus,
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
    vec,
};
use tokio::sync::mpsc::UnboundedSender;

const NODE_START_INTERVAL: usize = 10;
const NODE_STAT_UPDATE_INTERVAL: Duration = Duration::from_secs(5);
const NAT_DETECTION_SERVERS_LIST_URL: &str =
    "https://sn-testnet.s3.eu-west-2.amazonaws.com/nat-detection-servers";
/// If nat detection fails for more than 3 times, we don't want to waste time running during every node start.
const MAX_ERRORS_WHILE_RUNNING_NAT_DETECTION: usize = 3;

pub struct Home {
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
}

pub enum LockRegistryState {
    StartingNodes,
    StoppingNodes,
    ResettingNodes,
}

impl Home {
    pub async fn new(
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
            is_nat_status_determined: false,
            error_while_running_nat_detection: 0,
            node_stats: NodeStats::default(),
            node_stats_last_update: Instant::now(),
            nodes_to_start: allocated_disk_space,
            node_table_state: Default::default(),
            lock_registry: None,
            discord_username: discord_username.to_string(),
            safenode_path,
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
        home.load_node_registry_and_update_states()?;

        Ok(home)
    }

    /// Tries to trigger the update of node stats if the last update was more than `NODE_STAT_UPDATE_INTERVAL` ago.
    /// The result is sent via the HomeActions::NodesStatsObtained action.
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

impl Component for Home {
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
            Action::SwitchScene(scene) => match scene {
                Scene::Home => {
                    self.active = true;
                    // make sure we're in navigation mode
                    return Ok(Some(Action::SwitchInputMode(InputMode::Navigation)));
                }
                Scene::BetaProgramme
                | Scene::ManageNodes
                | Scene::HelpPopUp
                | Scene::ResetPopUp => self.active = true,
                _ => self.active = false,
            },
            Action::StoreNodesToStart(count) => {
                self.nodes_to_start = count;
                if self.nodes_to_start == 0 {
                    info!("Nodes to start set to 0. Sending command to stop all nodes.");
                    return Ok(Some(Action::HomeActions(HomeActions::StopNodes)));
                } else {
                    info!("Nodes to start set to: {count}. Sending command to start nodes");
                    return Ok(Some(Action::HomeActions(HomeActions::StartNodes)));
                }
            }
            Action::StoreDiscordUserName(username) => {
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
            Action::HomeActions(HomeActions::StartNodes) => {
                if self.lock_registry.is_some() {
                    error!("Registry is locked. Cannot start node now.");
                    return Ok(None);
                }

                if self.nodes_to_start == 0 {
                    info!("Nodes to start not set. Ask for input.");
                    return Ok(Some(Action::HomeActions(HomeActions::TriggerManageNodes)));
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
                    action_sender,
                );
            }
            Action::HomeActions(HomeActions::StopNodes) => {
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
            Action::HomeActions(HomeActions::ResetNodes) => {
                if self.lock_registry.is_some() {
                    error!("Registry is locked. Cannot reset nodes now.");
                    return Ok(None);
                }

                self.lock_registry = Some(LockRegistryState::ResettingNodes);
                let action_sender = self.get_actions_sender()?;
                info!("Got action to reset nodes");
                reset_nodes(action_sender, false);
            }

            Action::Tick => {
                self.try_update_node_stats(false)?;
            }
            Action::HomeActions(HomeActions::NodesStatsObtained(stats)) => {
                self.node_stats = stats;
            }
            Action::HomeActions(HomeActions::StartNodesCompleted)
            | Action::HomeActions(HomeActions::StopNodesCompleted) => {
                self.lock_registry = None;
                self.load_node_registry_and_update_states()?;
            }
            Action::HomeActions(HomeActions::ResetNodesCompleted { trigger_start_node }) => {
                self.lock_registry = None;
                self.load_node_registry_and_update_states()?;

                if trigger_start_node {
                    debug!("Reset nodes completed. Triggering start nodes.");
                    return Ok(Some(Action::HomeActions(HomeActions::StartNodes)));
                }
                debug!("Reset nodes completed");
            }
            Action::HomeActions(HomeActions::SuccessfullyDetectedNatStatus) => {
                debug!("Successfully detected nat status, is_nat_status_determined set to true");
                self.is_nat_status_determined = true;
            }
            Action::HomeActions(HomeActions::ErrorWhileRunningNatDetection) => {
                self.error_while_running_nat_detection += 1;
                debug!(
                    "Error while running nat detection. Error count: {}",
                    self.error_while_running_nat_detection
                );
            }
            // todo: should triggers go here? Make distinction between a component + a scene and how they interact.
            Action::HomeActions(HomeActions::TriggerBetaProgramme) => {
                return Ok(Some(Action::SwitchScene(Scene::BetaProgramme)));
            }
            Action::HomeActions(HomeActions::TriggerManageNodes) => {
                return Ok(Some(Action::SwitchScene(Scene::ManageNodes)));
            }
            Action::HomeActions(HomeActions::TriggerHelp) => {
                return Ok(Some(Action::SwitchScene(Scene::HelpPopUp)));
            }
            Action::HomeActions(HomeActions::TriggerResetNodesPopUp) => {
                return Ok(Some(Action::SwitchScene(Scene::ResetPopUp)));
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

        let layer_zero = Layout::new(
            Direction::Vertical,
            [
                // header
                Constraint::Max(1),
                // device status
                Constraint::Max(10),
                // node status
                Constraint::Min(3),
                // footer
                Constraint::Max(5),
            ],
        )
        .split(area);

        // ==== Header ====

        let layer_one_header = Layout::new(
            Direction::Horizontal,
            vec![Constraint::Min(40), Constraint::Fill(20)],
        )
        .split(layer_zero[0]);
        f.render_widget(
            Paragraph::new("Autonomi Node Launchpad")
                .alignment(Alignment::Left)
                .fg(LIGHT_PERIWINKLE),
            layer_one_header[0],
        );
        let discord_user_name_text = if self.discord_username.is_empty() {
            "".to_string()
        } else {
            format!("Discord Username: {} ", &self.discord_username)
        };
        f.render_widget(
            Paragraph::new(discord_user_name_text)
                .alignment(Alignment::Right)
                .fg(VERY_LIGHT_AZURE),
            layer_one_header[1],
        );

        // ==== Device Status =====

        if self.node_services.is_empty() {
            let line1 = Line::from(vec![Span::styled(
                "No Nodes on this device",
                Style::default().fg(GHOST_WHITE),
            )]);
            let line2 = Line::from(vec![
                Span::styled("Press ", Style::default().fg(GHOST_WHITE)),
                Span::styled("Ctrl+G", Style::default().fg(EUCALYPTUS)),
                Span::styled(
                    " to Add Nodes and get started.",
                    Style::default().fg(GHOST_WHITE),
                ),
            ]);
            f.render_widget(
                Paragraph::new(vec![line1, line2]).block(
                    Block::default()
                        .title("Device Status")
                        .title_style(Style::new().fg(GHOST_WHITE))
                        .borders(Borders::ALL)
                        .padding(Padding::uniform(1))
                        .border_style(Style::new().fg(VERY_LIGHT_AZURE)),
                ),
                layer_zero[1],
            );
        } else {
            // display stats as a table

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
            let total_nanos_earned_row = Row::new(vec![
                Cell::new("Total Nanos Earned".to_string()).fg(VIVID_SKY_BLUE),
                Cell::new(self.node_stats.forwarded_rewards.to_string())
                    .fg(VIVID_SKY_BLUE)
                    .bold(),
            ]);
            let stats_rows = vec![
                storage_allocated_row,
                memory_use_row.bottom_margin(2),
                total_nanos_earned_row,
            ];
            let stats_width = [Constraint::Max(25), Constraint::Min(5)];
            let stats_table = Table::new(stats_rows, stats_width).block(
                Block::default()
                    .title("Device Status")
                    .title_style(Style::default().fg(GHOST_WHITE))
                    .borders(Borders::ALL)
                    .padding(Padding::uniform(1))
                    .style(Style::default().fg(VERY_LIGHT_AZURE)),
            );
            f.render_widget(stats_table, layer_zero[1]);
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

                let row = vec![n.service_name.clone(), peer_id, status];
                let row_style = if n.status == ServiceStatus::Running {
                    Style::default().fg(EUCALYPTUS)
                } else {
                    Style::default().fg(GHOST_WHITE)
                };
                Some(Row::new(row).style(row_style))
            })
            .collect();

        if node_rows.is_empty() {
            f.render_widget(
                Paragraph::new("Nodes will appear here when added")
                    .fg(LIGHT_PERIWINKLE)
                    .block(
                        Block::default()
                            .title("Node Status")
                            .title_style(Style::default().fg(LIGHT_PERIWINKLE))
                            .borders(Borders::ALL)
                            .border_style(style::Style::default().fg(COOL_GREY))
                            .padding(Padding::uniform(1)),
                    ),
                layer_zero[2],
            );
        } else {
            let node_widths = [
                Constraint::Max(15),
                Constraint::Min(30),
                Constraint::Max(10),
            ];
            let table = Table::new(node_rows, node_widths)
                .column_spacing(2)
                .highlight_style(Style::new().reversed())
                .block(
                    Block::default()
                        .title("Node Status")
                        .padding(Padding::new(2, 2, 1, 1))
                        .title_style(Style::default().fg(GHOST_WHITE))
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(EUCALYPTUS)),
                )
                .highlight_symbol("*");
            f.render_stateful_widget(table, layer_zero[2], &mut self.node_table_state);
        }

        // ===== Popup =====

        if let Some(registry_state) = &self.lock_registry {
            let popup_area = centered_rect_fixed(50, 12, area);
            clear_area(f, popup_area);

            let popup_border = Paragraph::new("Manage Nodes").block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::new().fg(GHOST_WHITE)),
            );

            let popup_text = match registry_state {
                LockRegistryState::StartingNodes => {
                    if self.should_we_run_nat_detection() {
                        "Starting nodes...\nPlease wait, performing initial NAT detection\nThis may take a couple minutes."
                    } else {
                        "Starting nodes..."
                    }
                }
                LockRegistryState::StoppingNodes => "Stopping nodes...",
                LockRegistryState::ResettingNodes => "Resetting nodes...",
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
                .alignment(Alignment::Center)
                .fg(EUCALYPTUS);
            f.render_widget(text, centred_area);

            f.render_widget(popup_border, popup_area);
        }

        Ok(())
    }
}

fn stop_nodes(services: Vec<String>, action_sender: UnboundedSender<Action>) {
    tokio::task::spawn_local(async move {
        if let Err(err) =
            sn_node_manager::cmd::node::stop(vec![], services, VerbosityLevel::Minimal).await
        {
            error!("Error while stopping services {err:?}");
        } else {
            info!("Successfully stopped services");
        }
        if let Err(err) = action_sender.send(Action::HomeActions(HomeActions::StopNodesCompleted)) {
            error!("Error while sending action: {err:?}");
        }
    });
}

async fn run_nat_detection_process() -> Result<()> {
    let servers = get_bootstrap_peers_from_url(NAT_DETECTION_SERVERS_LIST_URL.parse()?).await?;
    let servers = servers
        .choose_multiple(&mut rand::thread_rng(), 10)
        .cloned()
        .collect::<Vec<_>>();
    info!("Running nat detection with servers: {servers:?}");
    sn_node_manager::cmd::nat_detection::run_nat_detection(
        servers,
        true,
        None,
        None,
        Some("0.1.0".to_string()),
        VerbosityLevel::Minimal,
    )
    .await?;
    Ok(())
}

fn maintain_n_running_nodes(
    count: u16,
    owner: String,
    peers_args: PeersArgs,
    run_nat_detection: bool,
    safenode_path: Option<PathBuf>,
    action_sender: UnboundedSender<Action>,
) {
    tokio::task::spawn_local(async move {
        if run_nat_detection {
            if let Err(err) = run_nat_detection_process().await {
                error!("Error while running nat detection {err:?}. Registering the error.");
                if let Err(err) = action_sender.send(Action::HomeActions(
                    HomeActions::ErrorWhileRunningNatDetection,
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
            count,
            None,
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
            NODE_START_INTERVAL as u64,
        )
        .await
        {
            error!("Error while maintaining {count:?} running nodes {err:?}");
        } else {
            info!("Maintained {count} running nodes successfully.");
        }
        if let Err(err) = action_sender.send(Action::HomeActions(HomeActions::StartNodesCompleted))
        {
            error!("Error while sending action: {err:?}");
        }
    });
}

fn reset_nodes(action_sender: UnboundedSender<Action>, start_nodes_after_reset: bool) {
    tokio::task::spawn_local(async move {
        if let Err(err) = sn_node_manager::cmd::node::reset(true, VerbosityLevel::Minimal).await {
            error!("Error while resetting services {err:?}");
        } else {
            info!("Successfully reset services");
        }
        if let Err(err) =
            action_sender.send(Action::HomeActions(HomeActions::ResetNodesCompleted {
                trigger_start_node: start_nodes_after_reset,
            }))
        {
            error!("Error while sending action: {err:?}");
        }
    });
}
