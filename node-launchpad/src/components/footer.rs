// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::Component;
use crate::{
    action::{Action, FooterActions},
    mode::Scene,
    style::{GRAY, RED, TEAL, WHITE},
};
use color_eyre::eyre::Result;
use ratatui::{prelude::*, widgets::*};

#[derive(Default)]
pub struct Footer {
    current_scene: Scene,
    nodes_exist: bool,
}

impl Component for Footer {
    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::SwitchScene(scene) => {
                self.current_scene = scene;
            }
            Action::FooterActions(FooterActions::AtleastOneNodePresent(nodes_exist)) => {
                self.nodes_exist = nodes_exist;
            }
            _ => {}
        }
        Ok(None)
    }

    fn draw(&mut self, f: &mut crate::tui::Frame<'_>, area: Rect) -> Result<()> {
        let layer_zero = Layout::new(
            Direction::Vertical,
            [
                // for the rest of the home scene
                Constraint::Min(1),
                // our footer
                Constraint::Max(5),
            ],
        )
        .split(area);
        let border = Paragraph::new("").block(
            Block::default()
                .title("Available Commands")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(RED)),
        );

        let layer_one = Layout::new(
            Direction::Vertical,
            [
                // border
                Constraint::Length(1),
                // line1
                Constraint::Length(2),
                // line2
                Constraint::Length(1),
                // border
                Constraint::Length(1),
            ],
        )
        .split(layer_zero[1]);

        let text_style = if self.nodes_exist {
            Style::default().fg(RED)
        } else {
            Style::default().fg(GRAY)
        };

        let command_style = if self.nodes_exist {
            Style::default().fg(WHITE)
        } else {
            Style::default().fg(GRAY)
        };

        let (line1, line2) = match self.current_scene {
            Scene::Home => {
                let line1 = Line::from(vec![
                    Span::styled(" [Ctrl+S] ", command_style),
                    Span::styled("Start all Nodes       ", text_style),
                    Span::styled("[Ctrl+X] ", command_style),
                    Span::styled("Stop all Nodes         ", text_style),
                    Span::styled("[H] ", Style::default().fg(WHITE)),
                    Span::styled("Help", Style::default().fg(RED)),
                ]);

                let line2 = Line::from(vec![
                    Span::styled(" [Ctrl+G] ", Style::default().fg(WHITE)),
                    Span::styled("Manage Nodes          ", Style::default().fg(RED)),
                    Span::styled("[Ctrl+B] ", Style::default().fg(WHITE)),
                    Span::styled("Beta Rewards Programme ", Style::default().fg(TEAL)),
                    Span::styled("[Q] ", Style::default().fg(WHITE)),
                    Span::styled("Quit", Style::default().fg(RED)),
                ]);

                (line1, line2)
            }
            Scene::Options => (Line::from("none"), Line::from("none")),
            Scene::DiscordUsernameInputBox => {
                let line1 = Line::from(" ⏎ Accept, [Esc] Cancel");
                let line2 = Line::from(" ⏎ Accept, [Esc] Cancel");

                (line1, line2)
            }
            Scene::ResourceAllocationInputBox => {
                let line1 = Line::from(" ⏎ Accept, [Esc] Cancel.");
                let line2 = Line::from(" ⏎ Accept, [Esc] Cancel.");
                (line1, line2)
            }
        };

        f.render_widget(Paragraph::new(line1), layer_one[1]);
        f.render_widget(Paragraph::new(line2), layer_one[2]);
        // render the border after the text so we don't get white spaces at the left border
        f.render_widget(border, layer_zero[1]);

        Ok(())
    }
}
