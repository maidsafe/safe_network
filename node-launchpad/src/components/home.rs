// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{
    manage_nodes::{GB, MB},
    utils::centered_rect_fixed,
    Component, Frame,
};
use crate::{
    action::{Action, FooterActions, HomeActions},
    config::Config,
    mode::{InputMode, Scene},
    style::{COOL_GREY, EUCALYPTUS, GHOST_WHITE, VERY_LIGHT_AZURE},
};
use color_eyre::eyre::{OptionExt, Result};
use fs_extra::dir::get_size;
use futures::StreamExt;
use rand::seq::SliceRandom;
use ratatui::{prelude::*, widgets::*};
use sn_node_manager::{config::get_node_registry_path, VerbosityLevel};
use sn_peers_acquisition::{get_bootstrap_peers_from_url, PeersArgs};
use sn_service_management::{
    rpc::{RpcActions, RpcClient},
    NodeRegistry, NodeServiceData, ServiceStatus,
};
use std::{
    net::SocketAddr,
    path::PathBuf,
    time::{Duration, Instant},
};
use tokio::sync::mpsc::UnboundedSender;

const NODE_START_INTERVAL: usize = 10;
const NODE_STAT_UPDATE_INTERVAL: Duration = Duration::from_secs(15);
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
    node_stats: NodesStats,
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
            is_nat_status_determined: false,
            error_while_running_nat_detection: 0,
            node_stats: NodesStats::new(),
            nodes_to_start: allocated_disk_space,
            node_table_state: Default::default(),
            lock_registry: Default::default(),
            discord_username: discord_username.to_string(),
            safenode_path,
        };
        home.load_node_registry_and_update_states()?;

        Ok(home)
    }

    /// Tries to trigger the update of node stats if the last update was more than `NODE_STAT_UPDATE_INTERVAL` ago.
    /// The result is sent via the HomeActions::NodesStatsObtained action.
    fn try_update_node_stats(&mut self, force_update: bool) -> Result<()> {
        if self.node_stats.last_update.elapsed() > NODE_STAT_UPDATE_INTERVAL || force_update {
            self.node_stats.last_update = Instant::now();

            NodesStats::fetch_all_node_stats(&self.node_services, self.get_actions_sender()?);
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

        if let Some(action_sender) = self.action_sender.as_ref() {
            if let Err(err) = action_sender.send(Action::FooterActions(
                FooterActions::AtleastOneNodePresent(!self.node_services.is_empty()),
            )) {
                error!("Error while sending action: {err:?}");
            }
        }

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

        // update the footer as soon as the app is run
        if let Some(action_sender) = self.action_sender.as_ref() {
            if let Err(err) = action_sender.send(Action::FooterActions(
                FooterActions::AtleastOneNodePresent(!self.node_services.is_empty()),
            )) {
                error!("Error while sending action: {err:?}");
            }
        }
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
                Scene::BetaProgramme | Scene::ManageNodes | Scene::HelpPopUp => self.active = true,
                _ => self.active = false,
            },
            Action::StoreNodesToStart(count) => {
                self.nodes_to_start = count;
            }
            Action::StoreDiscordUserName(username) => {
                let reset_safenode_services = (self.discord_username != username)
                    && !self.discord_username.is_empty()
                    && !self.node_services.is_empty();
                self.discord_username = username;

                // todo: The discord_username popup should warn people that if nodes are running, they will be reset.
                // And the earnings will be lost.
                if reset_safenode_services {
                    self.lock_registry = Some(LockRegistryState::ResettingNodes);
                    info!("Resetting safenode services because the discord username was reset.");
                    let action_sender = self.get_actions_sender()?;
                    reset_nodes(action_sender);
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
            Action::Tick => {
                self.try_update_node_stats(false)?;
            }
            Action::HomeActions(HomeActions::NodesStatsObtained {
                wallet_balance,
                space_used,
            }) => {
                self.node_stats.wallet_balance = wallet_balance;
                self.node_stats.space_used = space_used;
            }
            Action::HomeActions(HomeActions::StartNodesCompleted)
            | Action::HomeActions(HomeActions::StopNodesCompleted) => {
                self.lock_registry = None;
                self.load_node_registry_and_update_states()?;
            }
            Action::HomeActions(HomeActions::ResetNodesCompleted) => {
                self.lock_registry = None;
                self.load_node_registry_and_update_states()?;

                // trigger start nodes.
                return Ok(Some(Action::HomeActions(HomeActions::StartNodes)));
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
                .style(Style::default().fg(COOL_GREY)),
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
                .style(Style::default().fg(VERY_LIGHT_AZURE)),
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
                        .title_style(Style::default().fg(GHOST_WHITE))
                        .borders(Borders::ALL)
                        .padding(Padding::uniform(1))
                        .style(Style::default().fg(VERY_LIGHT_AZURE)),
                ),
                layer_zero[1],
            );
        } else {
            // display stats as a table
            let (space_used_value, space_used_header) = {
                // if space used within 1GB, display in mb
                if self.node_stats.space_used as f64 / (MB as f64) < (MB as f64) {
                    (
                        format!("{:.2}", self.node_stats.space_used as f64 / MB as f64),
                        "Space Used (MB)".to_string(),
                    )
                } else {
                    // else display in gb
                    (
                        format!("{:.2}", self.node_stats.space_used as f64 / GB as f64),
                        "Space Used (GB)".to_string(),
                    )
                }
            };
            let stats_rows = vec![Row::new(vec![
                self.node_stats.wallet_balance.to_string(),
                space_used_value,
                self.node_stats.memory_usage.to_string(),
                self.node_stats.network_usage.to_string(),
            ])];
            let stats_width = [
                Constraint::Min(15),
                Constraint::Min(10),
                Constraint::Min(10),
                Constraint::Min(10),
            ];
            let stats_table = Table::new(stats_rows, stats_width)
                .column_spacing(2)
                .header(
                    Row::new(vec![
                        "Wallet Balance",
                        space_used_header.as_str(),
                        "Memory usage",
                        "Network Usage",
                    ])
                    .style(Style::new().bold().fg(GHOST_WHITE)),
                )
                .block(
                    Block::default()
                        .title("Device Status")
                        .title_style(Style::default().fg(GHOST_WHITE))
                        .borders(Borders::ALL)
                        .padding(Padding::uniform(1))
                        .style(Style::default().fg(VERY_LIGHT_AZURE)),
                );
            f.render_widget(stats_table, layer_zero[1]);
            // "todo: display a table".to_string()
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
                    .style(Style::default().fg(COOL_GREY))
                    .block(
                        Block::default()
                            .title("Node Status")
                            .title_style(Style::default().fg(COOL_GREY))
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
            f.render_widget(Clear, popup_area);
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
                .style(style::Style::default().fg(EUCALYPTUS));
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
            true,
            count,
            None,
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

fn reset_nodes(action_sender: UnboundedSender<Action>) {
    tokio::task::spawn_local(async move {
        if let Err(err) = sn_node_manager::cmd::node::reset(true, VerbosityLevel::Minimal).await {
            error!("Error while resetting services {err:?}");
        } else {
            info!("Successfully reset services");
        }
        if let Err(err) = action_sender.send(Action::HomeActions(HomeActions::ResetNodesCompleted))
        {
            error!("Error while sending action: {err:?}");
        }
    });
}

/// The stats of all the running nodes
/// todo: certain stats like wallet balance, space used can be calculated even if the node is offline.
struct NodesStats {
    pub wallet_balance: u64,
    pub space_used: u64,
    pub memory_usage: usize,
    pub network_usage: usize,

    // pub system_info: sysinfo::System,
    pub last_update: Instant,
}

impl NodesStats {
    pub fn new() -> Self {
        Self {
            wallet_balance: 0,
            space_used: 0,
            memory_usage: 0,
            network_usage: 0,
            // system_info: sysinfo::System::new_all(),
            last_update: Instant::now(),
        }
    }

    pub fn fetch_all_node_stats(nodes: &[NodeServiceData], action_sender: UnboundedSender<Action>) {
        let node_details = nodes
            .iter()
            .filter_map(|node| {
                if node.status == ServiceStatus::Running {
                    Some((
                        node.service_name.clone(),
                        node.rpc_socket_addr,
                        node.data_dir_path.clone(),
                    ))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        tokio::task::spawn_local(async move {
            Self::fetch_all_node_stats_inner(node_details, action_sender).await;
        });
    }

    async fn fetch_all_node_stats_inner(
        node_details: Vec<(String, SocketAddr, PathBuf)>,
        action_sender: UnboundedSender<Action>,
    ) {
        let mut stream = futures::stream::iter(node_details)
            .map(|(service_name, rpc_addr, data_dir)| async move {
                (
                    Self::fetch_stat_per_node(rpc_addr, data_dir).await,
                    service_name,
                )
            })
            .buffer_unordered(5);

        let mut all_wallet_balance = 0;
        let mut all_space_used = 0;

        while let Some((result, service_name)) = stream.next().await {
            match result {
                Ok((wallet_balance, space_used)) => {
                    info!("Wallet balance: {wallet_balance}, Space used: {space_used}");
                    all_wallet_balance += wallet_balance;
                    all_space_used += space_used;
                }
                Err(err) => {
                    error!("Error while fetching stats from {service_name:?}: {err:?}");
                }
            }
        }

        if let Err(err) = action_sender.send(Action::HomeActions(HomeActions::NodesStatsObtained {
            wallet_balance: all_wallet_balance,
            space_used: all_space_used,
        })) {
            error!("Error while sending action: {err:?}");
        }
    }

    // todo: get all the stats
    async fn fetch_stat_per_node(rpc_addr: SocketAddr, data_dir: PathBuf) -> Result<(u64, u64)> {
        let now = Instant::now();
        let rpc_client = RpcClient::from_socket_addr(rpc_addr);
        let wallet_balance = rpc_client.node_info().await?.wallet_balance;

        let space_used = get_size(data_dir)?;

        debug!("Fetched stats from {rpc_addr:?} in {:?}", now.elapsed());
        Ok((wallet_balance, space_used))
    }
}
