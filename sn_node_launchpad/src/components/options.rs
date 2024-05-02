// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::Component;
use crate::{
    action::Action,
    mode::{InputMode, Scene},
};
use color_eyre::Result;
use crossterm::event::{Event, KeyCode, KeyEvent};
use ratatui::{prelude::*, widgets::*};
use tui_input::{backend::crossterm::EventHandler, Input};

#[derive(Default)]
pub struct Options {
    // state
    show_scene: bool,
    input_mode: InputMode,
    input: Input,
}

impl Component for Options {
    fn handle_key_events(&mut self, key: KeyEvent) -> Result<Option<Action>> {
        // while in entry mode, keybinds are not captured, so gotta exit entry mode from here
        match key.code {
            KeyCode::Esc => {
                return Ok(Some(Action::SwitchInputMode(InputMode::Navigation)));
            }
            KeyCode::Down => {
                // self.select_next_input_field();
            }
            KeyCode::Up => {
                // self.select_previous_input_field();
            }
            _ => {}
        }
        self.input.handle_event(&Event::Key(key));
        Ok(None)
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::SwitchScene(scene) => match scene {
                Scene::Options => self.show_scene = true,
                _ => self.show_scene = false,
            },
            Action::SwitchInputMode(mode) => self.input_mode = mode,
            _ => {}
        };
        Ok(None)
    }

    fn draw(&mut self, f: &mut crate::tui::Frame<'_>, area: Rect) -> Result<()> {
        if !self.show_scene {
            return Ok(());
        }

        // index 0 is reserved for tab; 2 is for keybindings
        let layer_zero = Layout::new(
            Direction::Vertical,
            [Constraint::Max(1), Constraint::Min(15), Constraint::Max(3)],
        )
        .split(area);

        // break the index 1 into sub sections
        let layer_one = Layout::new(
            Direction::Vertical,
            [
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
            ],
        )
        .split(layer_zero[1]);

        let input = Paragraph::new(self.input.value())
            .style(Style::default())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Peer MultiAddress"),
            );
        f.render_widget(input, layer_one[0]);
        let input = Paragraph::new(self.input.value())
            .style(Style::default())
            .block(Block::default().borders(Borders::ALL).title("Home Network"));
        f.render_widget(input, layer_one[1]);
        let input = Paragraph::new(self.input.value())
            .style(Style::default())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Data dir Path"),
            );
        f.render_widget(input, layer_one[2]);
        let input = Paragraph::new(self.input.value())
            .style(Style::default())
            .block(Block::default().borders(Borders::ALL).title("Log dir Path"));
        f.render_widget(input, layer_one[3]);
        let input = Paragraph::new(self.input.value())
            .style(Style::default())
            .block(Block::default().borders(Borders::ALL).title("Node Version"));
        f.render_widget(input, layer_one[4]);
        let input = Paragraph::new(self.input.value())
            .style(Style::default())
            .block(Block::default().borders(Borders::ALL).title("RPC Address"));
        f.render_widget(input, layer_one[5]);

        Ok(())
    }
}
