// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::super::{utils::centered_rect_fixed, Component};
use crate::{
    action::Action,
    mode::{InputMode, Scene},
    style::{clear_area, EUCALYPTUS, GHOST_WHITE, VIVID_SKY_BLUE},
    tui::Frame,
    widgets::hyperlink::Hyperlink,
};
use color_eyre::eyre::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    prelude::{Rect, *},
    widgets::*,
};

#[derive(Default)]
pub struct HelpPopUp {
    /// Whether the component is active right now, capturing keystrokes + drawing things.
    active: bool,
}

impl Component for HelpPopUp {
    fn handle_key_events(&mut self, key: KeyEvent) -> Result<Vec<Action>> {
        if !self.active {
            return Ok(vec![]);
        }

        let send_back = match key.code {
            KeyCode::Esc => {
                debug!("Got Esc, exiting HelpPopUp");
                vec![Action::SwitchScene(Scene::Home)]
            }
            _ => {
                vec![]
            }
        };

        Ok(send_back)
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        let send_back = match action {
            Action::SwitchScene(scene) => match scene {
                Scene::HelpPopUp => {
                    self.active = true;
                    Some(Action::SwitchInputMode(InputMode::Entry))
                }
                _ => {
                    self.active = false;
                    None
                }
            },
            _ => None,
        };
        Ok(send_back)
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        if !self.active {
            return Ok(());
        }

        let layer_zero = centered_rect_fixed(50, 12, area);

        let layer_one = Layout::new(
            Direction::Vertical,
            [
                // for the layer 0 border
                Constraint::Length(2),
                // lines
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                // dash
                Constraint::Min(1),
                // button
                Constraint::Length(1),
                Constraint::Length(1),
            ],
        )
        .split(layer_zero);
        clear_area(f, layer_zero);

        let pop_up_border = Paragraph::new("").block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Get Help ")
                .title_style(style::Style::default().fg(EUCALYPTUS))
                .border_style(Style::new().fg(EUCALYPTUS)),
        );

        let line1 = Paragraph::new(" See the quick start guides:")
            .block(Block::default().padding(Padding::horizontal(1)));
        f.render_widget(line1.fg(GHOST_WHITE), layer_one[1]);

        let link1 = Hyperlink::new(
            Span::styled(
                "  https://autonomi.com/getting-started",
                Style::default().fg(VIVID_SKY_BLUE),
            ),
            "https://autonomi.com/getting-started",
        );
        f.render_widget_ref(link1, layer_one[2]);

        let line2 = Paragraph::new(" Get direct help via Discord:")
            .fg(GHOST_WHITE)
            .block(Block::default().padding(Padding::horizontal(1)));
        f.render_widget(line2, layer_one[3]);

        let link2 = Hyperlink::new(
            Span::styled(
                "  https://discord.gg/autonomi",
                Style::default().fg(VIVID_SKY_BLUE),
            ),
            "https://discord.gg/autonomi",
        );
        f.render_widget_ref(link2, layer_one[4]);

        let line3 = Paragraph::new(" To join the Beta Rewards Program:")
            .fg(GHOST_WHITE)
            .block(Block::default().padding(Padding::horizontal(1)));
        f.render_widget(line3, layer_one[5]);
        let link3 = Hyperlink::new(
            Span::styled(
                "  https://autonomi.com/beta",
                Style::default().fg(VIVID_SKY_BLUE),
            ),
            "https://autonomi.com/beta",
        );
        f.render_widget_ref(link3, layer_one[6]);

        let dash = Block::new()
            .borders(Borders::BOTTOM)
            .border_style(Style::new().fg(GHOST_WHITE));
        f.render_widget(dash, layer_one[7]);

        let button = Paragraph::new("  Close [Esc]").style(Style::default().fg(GHOST_WHITE));
        f.render_widget(button, layer_one[8]);

        f.render_widget(pop_up_border, layer_zero);

        Ok(())
    }
}
