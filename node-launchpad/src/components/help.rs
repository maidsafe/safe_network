use super::header::SelectedMenuItem;
use color_eyre::eyre::Result;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Padding, Row, Table},
    Frame,
};
use tokio::sync::mpsc::UnboundedSender;

use super::Component;
use crate::{
    action::Action,
    components::header::Header,
    mode::{InputMode, Scene},
    style::{EUCALYPTUS, GHOST_WHITE, VERY_LIGHT_AZURE, VIVID_SKY_BLUE},
    widgets::hyperlink::Hyperlink,
};
use ansi_to_tui::IntoText;

#[derive(Clone)]
pub struct Help {
    pub active: bool,
    pub action_tx: Option<UnboundedSender<Action>>,
}

impl Help {
    pub async fn new() -> Result<Self> {
        Ok(Self {
            active: false,
            action_tx: None,
        })
    }
}

impl Component for Help {
    fn init(&mut self, _area: Rect) -> Result<()> {
        Ok(())
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        if !self.active {
            return Ok(());
        }
        // We define a layout, top and down box.
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Length(1),
                Constraint::Min(7),
                Constraint::Max(9),
            ])
            .split(area);

        // ==== Header =====
        let header = Header::new();
        f.render_stateful_widget(header, layout[0], &mut SelectedMenuItem::Help);

        // ---- Get Help & Support ----
        // Links

        let quickstart_guide_link = Hyperlink::new(
            "docs.autonomi.com/getstarted",
            "https://docs.autonomi.com/getstarted",
        );
        let beta_rewards_link = Hyperlink::new("autonomi.com/beta", "https://autonomi.com/beta");
        let get_direct_support_link =
            Hyperlink::new("autonomi.com/support", "https://autonomi.com/support");
        let download_latest_link =
            Hyperlink::new("autonomi.com/downloads", "https://autonomi.com/downloads");

        // Content
        let rows_help_and_support = vec![
            Row::new(vec![
                Cell::from(Line::from(vec![Span::styled(
                    "See the quick start guides:",
                    Style::default().fg(GHOST_WHITE),
                )])),
                Cell::from(Line::from(vec![Span::styled(
                    "To join the Beta Rewards Program:",
                    Style::default().fg(GHOST_WHITE),
                )])),
            ]),
            Row::new(vec![
                Cell::from(
                    quickstart_guide_link
                        .to_string()
                        .into_text()
                        .unwrap()
                        .clone(),
                ),
                Cell::from(beta_rewards_link.to_string().into_text().unwrap().clone()),
            ]),
            Row::new(vec![
                // Empty row for padding
                Cell::from(Span::raw(" ")),
                Cell::from(Span::raw(" ")),
            ]),
            Row::new(vec![
                Cell::from(Line::from(vec![Span::styled(
                    "Get Direct Support:",
                    Style::default().fg(GHOST_WHITE),
                )])),
                Cell::from(Line::from(vec![Span::styled(
                    "Download the latest launchpad:",
                    Style::default().fg(GHOST_WHITE),
                )])),
            ]),
            Row::new(vec![
                Cell::from(
                    get_direct_support_link
                        .to_string()
                        .into_text()
                        .unwrap()
                        .clone(),
                ),
                Cell::from(
                    download_latest_link
                        .to_string()
                        .into_text()
                        .unwrap()
                        .clone(),
                ),
            ]),
        ];

        let table_help_and_support = Table::new(
            rows_help_and_support,
            vec![Constraint::Percentage(50), Constraint::Percentage(50)],
        )
        .block(
            Block::new()
                .borders(Borders::ALL)
                .padding(Padding::uniform(1))
                .title(" Get Help & Support ")
                .title_style(Style::default().bold()),
        );

        f.render_widget(table_help_and_support, layout[1]);

        // ---- Keyboard shortcuts ----
        let rows_keyboard_shortcuts = vec![
            Row::new(vec![
                Cell::from(Line::from(vec![
                    Span::styled("[S] ", Style::default().fg(GHOST_WHITE)),
                    Span::styled("Status", Style::default().fg(VIVID_SKY_BLUE)),
                ])),
                Cell::from(Line::from(vec![
                    Span::styled("[Ctrl+G] ", Style::default().fg(GHOST_WHITE)),
                    Span::styled("Manage Nodes", Style::default().fg(EUCALYPTUS)),
                ])),
                Cell::from(Line::from(vec![
                    Span::styled("[Ctrl+D] ", Style::default().fg(GHOST_WHITE)),
                    Span::styled(
                        "Change Storage Drive",
                        Style::default().fg(VERY_LIGHT_AZURE),
                    ),
                ])),
            ]),
            Row::new(vec![
                // Empty row for padding
                Cell::from(Span::raw(" ")),
                Cell::from(Span::raw(" ")),
                Cell::from(Span::raw(" ")),
            ]),
            Row::new(vec![
                Cell::from(Line::from(vec![
                    Span::styled("[O] ", Style::default().fg(GHOST_WHITE)),
                    Span::styled("Options", Style::default().fg(VIVID_SKY_BLUE)),
                ])),
                Cell::from(Line::from(vec![
                    Span::styled("[Ctrl+S] ", Style::default().fg(GHOST_WHITE)),
                    Span::styled("Start Nodes", Style::default().fg(EUCALYPTUS)),
                ])),
                Cell::from(Line::from(vec![
                    Span::styled("[Ctrl+B] ", Style::default().fg(GHOST_WHITE)),
                    Span::styled(
                        "Edit Discord Username",
                        Style::default().fg(VERY_LIGHT_AZURE),
                    ),
                ])),
            ]),
            Row::new(vec![
                // Empty row for padding
                Cell::from(Span::raw(" ")),
                Cell::from(Span::raw(" ")),
                Cell::from(Span::raw(" ")),
            ]),
            Row::new(vec![
                Cell::from(Line::from(vec![
                    Span::styled("[H] ", Style::default().fg(GHOST_WHITE)),
                    Span::styled("Help", Style::default().fg(VIVID_SKY_BLUE)),
                ])),
                Cell::from(Line::from(vec![
                    Span::styled("[Ctrl+X] ", Style::default().fg(GHOST_WHITE)),
                    Span::styled("Stop Nodes", Style::default().fg(EUCALYPTUS)),
                ])),
                Cell::from(Line::from(vec![
                    Span::styled("[Ctrl+L] ", Style::default().fg(GHOST_WHITE)),
                    Span::styled("Open Logs Folder", Style::default().fg(VERY_LIGHT_AZURE)),
                ])),
            ]),
            Row::new(vec![
                // Empty row for padding
                Cell::from(Span::raw(" ")),
                Cell::from(Span::raw(" ")),
                Cell::from(Span::raw(" ")),
            ]),
            Row::new(vec![
                Cell::from(Line::from(vec![
                    Span::styled("[Q] ", Style::default().fg(GHOST_WHITE)),
                    Span::styled("Quit", Style::default().fg(VIVID_SKY_BLUE)),
                ])),
                Cell::from(Line::from(vec![
                    Span::styled("[Ctrl+R] ", Style::default().fg(GHOST_WHITE)),
                    Span::styled("Reset All Nodes", Style::default().fg(EUCALYPTUS)),
                ])),
                Cell::from(""),
            ]),
        ];

        let table_keyboard_shortcuts = Table::new(
            rows_keyboard_shortcuts,
            vec![
                Constraint::Percentage(33),
                Constraint::Percentage(33),
                Constraint::Percentage(33),
            ],
        )
        .block(
            Block::new()
                .borders(Borders::ALL)
                .padding(Padding::uniform(1))
                .title(" Keyboard Shortcuts ")
                .title_style(Style::default().bold()),
        );

        f.render_widget(table_keyboard_shortcuts, layout[2]);

        Ok(())
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        if let Action::SwitchScene(scene) = action {
            if let Scene::Help = scene {
                self.active = true;
                // make sure we're in navigation mode
                return Ok(Some(Action::SwitchInputMode(InputMode::Navigation)));
            } else {
                self.active = false;
            }
        }
        Ok(None)
    }
}
