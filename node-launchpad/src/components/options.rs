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
    mode::{InputMode, Scene},
    style::{EUCALYPTUS, GHOST_WHITE, LIGHT_PERIWINKLE, VERY_LIGHT_AZURE, VIVID_SKY_BLUE},
    system,
};
use sn_node_manager::config::get_service_log_dir_path;

#[derive(Clone)]
pub struct Options {
    pub storage_mountpoint: PathBuf,
    pub storage_drive: String,
    pub discord_username: String,
    pub active: bool,
    pub action_tx: Option<UnboundedSender<Action>>,
}

impl Options {
    pub async fn new(
        storage_mountpoint: PathBuf,
        storage_drive: String,
        discord_username: String,
    ) -> Result<Self> {
        Ok(Self {
            storage_mountpoint,
            storage_drive,
            discord_username,
            active: false,
            action_tx: None,
        })
    }
}

impl Component for Options {
    fn init(&mut self, _area: Rect) -> Result<()> {
        Ok(())
    }

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
                    Constraint::Length(5),
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
            .title(" Storage Drive ")
            .title_style(Style::default().bold().fg(GHOST_WHITE))
            .style(Style::default().fg(GHOST_WHITE))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(VIVID_SKY_BLUE));
        let storage_drivename = Table::new(
            vec![
                Row::new(vec![
                    Cell::from(Span::raw(" ")), // Empty row for padding
                    Cell::from(Span::raw(" ")),
                ]),
                Row::new(vec![
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
            ],
            &[Constraint::Percentage(50), Constraint::Percentage(50)],
        )
        .block(block1)
        .style(Style::default().fg(GHOST_WHITE));

        // Beta Rewards Program — Discord Username
        let block2 = Block::default()
            .title(" Beta Rewards Program — Discord Username ")
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
                ]),
                Row::new(vec![
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
            &[Constraint::Percentage(50), Constraint::Percentage(50)],
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
                | Scene::BetaProgrammePopUp
                | Scene::ResetNodesPopUp
                | Scene::ChangeDrivePopUp => {
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
