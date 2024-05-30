// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{
    resource_allocation::{GB, MB},
    utils::centered_rect_fixed,
    Component, Frame,
};
use crate::{
    action::{Action, FooterActions, HomeActions},
    components::resource_allocation::GB_PER_NODE,
    config::Config,
    mode::{InputMode, Scene},
    style::{COOL_GREY, EUCALYPTUS, GHOST_WHITE, VERY_LIGHT_AZURE},
};
use color_eyre::eyre::{OptionExt, Result};
use fs_extra::dir::get_size;
use futures::StreamExt;
use ratatui::{prelude::*, widgets::*};
use sn_node_manager::{config::get_node_registry_path, VerbosityLevel};
use sn_peers_acquisition::PeersArgs;
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

pub struct Home {
    /// Whether the component is active right now, capturing keystrokes + drawing things.
    active: bool,
    action_sender: Option<UnboundedSender<Action>>,
    config: Config,
    // state
    node_services: Vec<NodeServiceData>,
    node_stats: NodesStats,
    node_table_state: TableState,
    allocated_disk_space: usize,
    discord_username: String,
    // Currently the node registry file does not support concurrent actions and thus can lead to
    // inconsistent state. Another solution would be to have a file lock/db.
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
            node_stats: NodesStats::new(),
            allocated_disk_space,
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
                Scene::DiscordUsernameInputBox | Scene::ResourceAllocationInputBox => {
                    self.active = true
                }
                _ => self.active = false,
            },
            Action::StoreAllocatedDiskSpace(space) => {
                self.allocated_disk_space = space;
            }
            Action::StoreDiscordUserName(username) => {
                let reset_safenode_services = (self.discord_username != username)
                    && !self.discord_username.is_empty()
                    && !self.node_services.is_empty();
                self.discord_username = username;

                // todo: The discord_username popup should warn people that if nodes are running, they will be reset.
                // And the earnings will be lost.
                if reset_safenode_services {
                    self.lock_registry = true;
                    info!("Resetting safenode services because the discord username was reset.");
                    let action_sender = self.get_actions_sender()?;
                    reset_nodes(action_sender);
                }
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
                self.lock_registry = false;
                self.load_node_registry_and_update_states()?;
            }
            Action::HomeActions(HomeActions::ResetNodesCompleted) => {
                self.lock_registry = false;
                self.load_node_registry_and_update_states()?;

                // trigger start nodes.
                return Ok(Some(Action::HomeActions(HomeActions::StartNodes)));
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
        let popup_area = centered_rect_fixed(25, 3, area);

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
