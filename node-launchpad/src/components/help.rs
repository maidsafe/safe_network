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
    style::{COOL_GREY, GHOST_WHITE},
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
            .constraints(vec![Constraint::Length(1), Constraint::Length(9)])
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
                .border_style(Style::default().fg(COOL_GREY))
                .padding(Padding::uniform(1))
                .title(" Get Help & Support ")
                .bold()
                .title_style(Style::default().bold().fg(GHOST_WHITE)),
        );

        f.render_widget(table_help_and_support, layout[1]);

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
