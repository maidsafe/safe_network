// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{utils::centered_rect, Component};
use crate::{
    action::Action,
    mode::{InputMode, Scene},
};
use color_eyre::Result;
use crossterm::event::{Event, KeyCode, KeyEvent};
use ratatui::{prelude::*, widgets::*};
use tui_input::{backend::crossterm::EventHandler, Input};

pub struct DiscordUsernameInputBox {
    show_scene: bool,
    discord_input_filed: Input,
    // cache the old value incase user presses Esc.
    old_value: String,
}

impl DiscordUsernameInputBox {
    pub fn new(username: String) -> Self {
        Self {
            show_scene: false,
            discord_input_filed: Input::default().with_value(username),
            old_value: Default::default(),
        }
    }
}

impl Component for DiscordUsernameInputBox {
    fn handle_key_events(&mut self, key: KeyEvent) -> Result<Vec<Action>> {
        // while in entry mode, keybinds are not captured, so gotta exit entry mode from here
        let send_back = match key.code {
            KeyCode::Enter => {
                let username = self.discord_input_filed.value().to_string();
                debug!("Got Enter, saving the discord username {username:?} and switching scene",);
                vec![
                    Action::StoreDiscordUserName(self.discord_input_filed.value().to_string()),
                    Action::SwitchScene(Scene::Home),
                ]
            }
            KeyCode::Esc => {
                debug!(
                    "Got Esc, restoring the old value {} and switching to home",
                    self.old_value
                );
                // reset to old value
                self.discord_input_filed = self
                    .discord_input_filed
                    .clone()
                    .with_value(self.old_value.clone());
                vec![Action::SwitchScene(Scene::Home)]
            }
            KeyCode::Char(' ') => vec![],
            KeyCode::Backspace => {
                // if max limit reached, we should allow Backspace to work.
                self.discord_input_filed.handle_event(&Event::Key(key));
                vec![]
            }
            _ => {
                // max 32 limit as per discord docs
                if self.discord_input_filed.value().len() >= 32 {
                    return Ok(vec![]);
                }
                self.discord_input_filed.handle_event(&Event::Key(key));
                vec![]
            }
        };
        Ok(send_back)
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        let send_back = match action {
            Action::SwitchScene(scene) => match scene {
                Scene::DiscordUsernameInputBox => {
                    self.show_scene = true;
                    self.old_value = self.discord_input_filed.value().to_string();
                    // set to entry input mode as we want to handle everything within our handle_key_events
                    // so by default if this scene is active, we capture inputs.
                    Some(Action::SwitchInputMode(InputMode::Entry))
                }
                _ => {
                    self.show_scene = false;
                    None
                }
            },
            _ => None,
        };
        Ok(send_back)
    }

    fn draw(&mut self, f: &mut crate::tui::Frame<'_>, area: Rect) -> Result<()> {
        if !self.show_scene {
            return Ok(());
        }

        // todo: y should be set to a min value, so it doesnt become too small
        let layer_zero = centered_rect(25, 15, area);

        let layer_one = Layout::new(
            Direction::Vertical,
            [
                // for the layer 0 border
                Constraint::Length(1),
                // for the input field
                Constraint::Min(1),
                // for buttons maybe? todo
                Constraint::Length(1),
            ],
        )
        .split(layer_zero);

        // layer zero
        let pop_up_border = Paragraph::new("").block(
            Block::default()
                .borders(Borders::ALL)
                .title("Enter Discord Username"),
        );
        f.render_widget(Clear, layer_zero);
        f.render_widget(pop_up_border, layer_zero);

        // layer one - 1
        let width = layer_one[1].width.max(3) - 3;

        let scroll = self.discord_input_filed.visual_scroll(width as usize);
        let input = Paragraph::new(self.discord_input_filed.value())
            .scroll((0, scroll as u16))
            // border left so that it doesn't conflict with layer_zero's border
            .block(Block::default().borders(Borders::LEFT));

        f.set_cursor(
            // Put cursor past the end of the input text
            layer_one[1].x
                + ((self.discord_input_filed.visual_cursor()).max(scroll) - scroll) as u16
                + 1,
            layer_one[1].y,
        );
        f.render_widget(input, layer_one[1]);

        Ok(())
    }
}
