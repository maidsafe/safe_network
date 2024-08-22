use std::path::PathBuf;

use color_eyre::eyre::{eyre, Result};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Row, Table},
    Frame,
};
use sn_releases::ReleaseType;
use tokio::sync::mpsc::UnboundedSender;

use super::{header::SelectedMenuItem, Component};
use crate::{
    action::{Action, OptionsActions},
    components::header::Header,
    connection_mode::ConnectionMode,
    mode::{InputMode, Scene},
    style::{
        COOL_GREY, EUCALYPTUS, GHOST_WHITE, LIGHT_PERIWINKLE, VERY_LIGHT_AZURE, VIVID_SKY_BLUE,
    },
    system,
};
use sn_node_manager::config::get_service_log_dir_path;

#[derive(Clone)]
pub struct Options {
    pub storage_mountpoint: PathBuf,
    pub storage_drive: String,
    pub discord_username: String,
    pub connection_mode: ConnectionMode,
    pub port_edit: bool,
    pub port_from: Option<u32>,
    pub port_to: Option<u32>,
    pub active: bool,
    pub action_tx: Option<UnboundedSender<Action>>,
}

impl Options {
    pub async fn new(
        storage_mountpoint: PathBuf,
        storage_drive: String,
        discord_username: String,
        connection_mode: ConnectionMode,
        port_from: Option<u32>,
        port_to: Option<u32>,
    ) -> Result<Self> {
        Ok(Self {
            storage_mountpoint,
            storage_drive,
            discord_username,
            connection_mode,
            port_edit: false,
            port_from,
            port_to,
            active: false,
            action_tx: None,
        })
    }
}

impl Component for Options {
    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        if !self.active {
            return Ok(());
        }
        // Define the layout to split the area into four sections
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Length(1),
                    Constraint::Length(9),
                    Constraint::Length(5),
                    Constraint::Length(5),
                    Constraint::Length(5),
                ]
                .as_ref(),
            )
            .split(area);

        // ==== Header =====
        let header = Header::new();
        f.render_stateful_widget(header, layout[0], &mut SelectedMenuItem::Options);

        // Storage Drive
        let block1 = Block::default()
            .title(" Device Options ")
            .title_style(Style::default().bold().fg(GHOST_WHITE))
            .style(Style::default().fg(GHOST_WHITE))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(VIVID_SKY_BLUE));
        let storage_drivename = Table::new(
            vec![
                Row::new(vec![
                    Cell::from(Span::raw(" ")), // Empty row for padding
                    Cell::from(Span::raw(" ")),
                    Cell::from(Span::raw(" ")),
                ]),
                Row::new(vec![
                    Cell::from(
                        Line::from(vec![Span::styled(
                            " Storage Drive: ",
                            Style::default().fg(LIGHT_PERIWINKLE),
                        )])
                        .alignment(Alignment::Left),
                    ),
                    Cell::from(
                        Line::from(vec![Span::styled(
                            format!(" {} ", self.storage_drive),
                            Style::default().fg(VIVID_SKY_BLUE),
                        )])
                        .alignment(Alignment::Left),
                    ),
                    Cell::from(
                        Line::from(vec![
                            Span::styled(" Change Drive ", Style::default().fg(VERY_LIGHT_AZURE)),
                            Span::styled(" [Ctrl+D] ", Style::default().fg(GHOST_WHITE)),
                        ])
                        .alignment(Alignment::Right),
                    ),
                ]),
                Row::new(vec![
                    Cell::from(Span::raw(" ")), // Empty row for padding
                    Cell::from(Span::raw(" ")),
                    Cell::from(Span::raw(" ")),
                ]),
                Row::new(vec![
                    Cell::from(
                        Line::from(vec![Span::styled(
                            " Connection Mode: ",
                            Style::default().fg(LIGHT_PERIWINKLE),
                        )])
                        .alignment(Alignment::Left),
                    ),
                    Cell::from(
                        Line::from(vec![Span::styled(
                            format!(" {} ", self.connection_mode),
                            Style::default().fg(VIVID_SKY_BLUE),
                        )])
                        .alignment(Alignment::Left),
                    ),
                    Cell::from(
                        Line::from(vec![
                            Span::styled(" Change Mode ", Style::default().fg(VERY_LIGHT_AZURE)),
                            Span::styled(" [Ctrl+K] ", Style::default().fg(GHOST_WHITE)),
                        ])
                        .alignment(Alignment::Right),
                    ),
                ]),
                Row::new(vec![
                    Cell::from(Span::raw(" ")), // Empty row for padding
                    Cell::from(Span::raw(" ")),
                    Cell::from(Span::raw(" ")),
                ]),
                Row::new(vec![
                    Cell::from(
                        Line::from(vec![Span::styled(
                            " Port Range: ",
                            Style::default().fg(LIGHT_PERIWINKLE),
                        )])
                        .alignment(Alignment::Left),
                    ),
                    Cell::from(
                        Line::from(vec![
                            if self.connection_mode == ConnectionMode::CustomPorts {
                                Span::styled(
                                    format!(
                                        " {}-{} ",
                                        self.port_from.unwrap_or(0),
                                        self.port_to.unwrap_or(0)
                                    ),
                                    Style::default().fg(VIVID_SKY_BLUE),
                                )
                            } else {
                                Span::styled(" Auto ", Style::default().fg(COOL_GREY))
                            },
                        ])
                        .alignment(Alignment::Left),
                    ),
                    Cell::from(
                        Line::from(vec![
                            Span::styled(
                                " Edit Port Range ",
                                if self.connection_mode == ConnectionMode::CustomPorts {
                                    Style::default().fg(VERY_LIGHT_AZURE)
                                } else {
                                    Style::default().fg(COOL_GREY)
                                },
                            ),
                            Span::styled(
                                " [Ctrl+P] ",
                                if self.connection_mode == ConnectionMode::CustomPorts {
                                    Style::default().fg(GHOST_WHITE)
                                } else {
                                    Style::default().fg(COOL_GREY)
                                },
                            ),
                        ])
                        .alignment(Alignment::Right),
                    ),
                ]),
            ],
            &[
                Constraint::Length(18),
                Constraint::Percentage(25),
                Constraint::Fill(1),
            ],
        )
        .block(block1)
        .style(Style::default().fg(GHOST_WHITE));

        // Beta Rewards Program
        let block2 = Block::default()
            .title(" Beta Rewards Program ")
            .title_style(Style::default().bold().fg(GHOST_WHITE))
            .style(Style::default().fg(GHOST_WHITE))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(VIVID_SKY_BLUE));
        let beta_rewards = Table::new(
            vec![
                Row::new(vec![
                    // Empty row for padding
                    Cell::from(Span::raw(" ")),
                    Cell::from(Span::raw(" ")),
                    Cell::from(Span::raw(" ")),
                ]),
                Row::new(vec![
                    Cell::from(
                        Line::from(vec![Span::styled(
                            " Discord Username: ",
                            Style::default().fg(LIGHT_PERIWINKLE),
                        )])
                        .alignment(Alignment::Left),
                    ),
                    Cell::from(
                        Line::from(vec![Span::styled(
                            format!(" {} ", self.discord_username),
                            Style::default().fg(VIVID_SKY_BLUE),
                        )])
                        .alignment(Alignment::Left),
                    ),
                    Cell::from(
                        Line::from(vec![
                            Span::styled(
                                " Edit Discord Username ",
                                Style::default().fg(VERY_LIGHT_AZURE),
                            ),
                            Span::styled(" [Ctrl+B] ", Style::default().fg(GHOST_WHITE)),
                        ])
                        .alignment(Alignment::Right),
                    ),
                ]),
            ],
            &[
                Constraint::Length(18),
                Constraint::Percentage(25),
                Constraint::Fill(1),
            ],
        )
        .block(block2)
        .style(Style::default().fg(GHOST_WHITE));

        // Access Logs
        let block3 = Block::default()
            .title(" Access Logs ")
            .title_style(Style::default().bold().fg(GHOST_WHITE))
            .style(Style::default().fg(GHOST_WHITE))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(VIVID_SKY_BLUE));
        let logs_folder = Table::new(
            vec![
                Row::new(vec![
                    // Empty row for padding
                    Cell::from(Span::raw(" ")),
                    Cell::from(Span::raw(" ")),
                ]),
                Row::new(vec![
                    Cell::from(
                        Line::from(vec![Span::styled(
                            " Open the Logs folder on this device ",
                            Style::default().fg(LIGHT_PERIWINKLE),
                        )])
                        .alignment(Alignment::Left),
                    ),
                    Cell::from(
                        Line::from(vec![
                            Span::styled(" Access Logs ", Style::default().fg(VERY_LIGHT_AZURE)),
                            Span::styled(" [Ctrl+L] ", Style::default().fg(GHOST_WHITE)),
                        ])
                        .alignment(Alignment::Right),
                    ),
                ]),
            ],
            &[Constraint::Percentage(50), Constraint::Percentage(50)],
        )
        .block(block3)
        .style(Style::default().fg(GHOST_WHITE));

        // Reset All Nodes
        let block4 = Block::default()
            .title(" Reset All Nodes ")
            .title_style(Style::default().bold().fg(GHOST_WHITE))
            .style(Style::default().fg(GHOST_WHITE))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(EUCALYPTUS));
        let reset_nodes = Table::new(
            vec![
                Row::new(vec![
                    Cell::from(Span::raw(" ")), // Empty row for padding
                    Cell::from(Span::raw(" ")),
                ]),
                Row::new(vec![
                    Cell::from(
                        Line::from(vec![Span::styled(
                            " Remove and Reset all Nodes on this device ",
                            Style::default().fg(LIGHT_PERIWINKLE),
                        )])
                        .alignment(Alignment::Left),
                    ),
                    Cell::from(
                        Line::from(vec![
                            Span::styled(" Begin Reset ", Style::default().fg(EUCALYPTUS)),
                            Span::styled(" [Ctrl+R] ", Style::default().fg(GHOST_WHITE)),
                        ])
                        .alignment(Alignment::Right),
                    ),
                ]),
            ],
            &[Constraint::Percentage(50), Constraint::Percentage(50)],
        )
        .block(block4)
        .style(Style::default().fg(GHOST_WHITE));

        // Render the tables in their respective sections
        f.render_widget(storage_drivename, layout[1]);
        f.render_widget(beta_rewards, layout[2]);
        f.render_widget(logs_folder, layout[3]);
        f.render_widget(reset_nodes, layout[4]);

        Ok(())
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::SwitchScene(scene) => match scene {
                Scene::Options
                | Scene::ChangeDrivePopUp
                | Scene::ChangeConnectionModePopUp
                | Scene::ChangePortsPopUp
                | Scene::BetaProgrammePopUp
                | Scene::ResetNodesPopUp => {
                    self.active = true;
                    // make sure we're in navigation mode
                    return Ok(Some(Action::SwitchInputMode(InputMode::Navigation)));
                }
                _ => self.active = false,
            },
            Action::OptionsActions(action) => match action {
                OptionsActions::TriggerChangeDrive => {
                    return Ok(Some(Action::SwitchScene(Scene::ChangeDrivePopUp)));
                }
                OptionsActions::UpdateStorageDrive(mountpoint, drive) => {
                    self.storage_mountpoint = mountpoint;
                    self.storage_drive = drive;
                }
                OptionsActions::TriggerChangeConnectionMode => {
                    return Ok(Some(Action::SwitchScene(Scene::ChangeConnectionModePopUp)));
                }
                OptionsActions::UpdateConnectionMode(mode) => {
                    self.connection_mode = mode;
                }
                OptionsActions::TriggerChangePortRange => {
                    return Ok(Some(Action::SwitchScene(Scene::ChangePortsPopUp)));
                }
                OptionsActions::UpdatePortRange(from, to) => {
                    self.port_from = Some(from);
                    self.port_to = Some(to);
                }
                OptionsActions::TriggerBetaProgramme => {
                    return Ok(Some(Action::SwitchScene(Scene::BetaProgrammePopUp)));
                }
                OptionsActions::UpdateBetaProgrammeUsername(username) => {
                    self.discord_username = username;
                }
                OptionsActions::TriggerAccessLogs => {
                    if let Err(e) = system::open_folder(
                        get_service_log_dir_path(ReleaseType::NodeLaunchpad, None, None)?
                            .to_str()
                            .ok_or_else(|| {
                                eyre!("We cannot get the log dir path for Node-Launchpad")
                            })?,
                    ) {
                        error!("Failed to open folder: {}", e);
                    }
                }
                OptionsActions::TriggerResetNodes => {
                    return Ok(Some(Action::SwitchScene(Scene::ResetNodesPopUp)))
                }
                _ => {}
            },
            _ => {}
        }
        Ok(None)
    }
}
