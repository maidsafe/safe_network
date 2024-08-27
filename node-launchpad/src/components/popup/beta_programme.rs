// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::super::utils::centered_rect_fixed;
use super::super::Component;
use crate::{
    action::{Action, OptionsActions},
    mode::{InputMode, Scene},
    style::{clear_area, EUCALYPTUS, GHOST_WHITE, LIGHT_PERIWINKLE, VIVID_SKY_BLUE},
    widgets::hyperlink::Hyperlink,
};
use color_eyre::Result;
use crossterm::event::{Event, KeyCode, KeyEvent};
use ratatui::{prelude::*, widgets::*};
use tui_input::{backend::crossterm::EventHandler, Input};

pub struct BetaProgramme {
    /// Whether the component is active right now, capturing keystrokes + draw things.
    active: bool,
    state: BetaProgrammeState,
    discord_input_filed: Input,
    // cache the old value incase user presses Esc.
    old_value: String,
}

enum BetaProgrammeState {
    DiscordIdAlreadySet,
    ShowTCs,
    RejectTCs,
    AcceptTCsAndEnterDiscordId,
}

impl BetaProgramme {
    pub fn new(username: String) -> Self {
        let state = if username.is_empty() {
            BetaProgrammeState::ShowTCs
        } else {
            BetaProgrammeState::DiscordIdAlreadySet
        };
        Self {
            active: false,
            state,
            discord_input_filed: Input::default().with_value(username),
            old_value: Default::default(),
        }
    }

    fn capture_inputs(&mut self, key: KeyEvent) -> Vec<Action> {
        let send_back = match key.code {
            KeyCode::Enter => {
                let username = self.discord_input_filed.value().to_string();

                if username.is_empty() {
                    debug!("Got Enter, but username is empty, ignoring.");
                    return vec![];
                }
                debug!(
                    "Got Enter, saving the discord username {username:?}  and switching to DiscordIdAlreadySet, and Home Scene",
                );
                self.state = BetaProgrammeState::DiscordIdAlreadySet;
                vec![
                    Action::StoreDiscordUserName(self.discord_input_filed.value().to_string()),
                    Action::OptionsActions(OptionsActions::UpdateBetaProgrammeUsername(username)),
                    Action::SwitchScene(Scene::Options),
                ]
            }
            KeyCode::Esc => {
                debug!(
                    "Got Esc, restoring the old value {} and switching to actual screen",
                    self.old_value
                );
                // reset to old value
                self.discord_input_filed = self
                    .discord_input_filed
                    .clone()
                    .with_value(self.old_value.clone());
                vec![Action::SwitchScene(Scene::Options)]
            }
            KeyCode::Char(' ') => vec![],
            KeyCode::Backspace => {
                // if max limit reached, we should allow Backspace to work.
                self.discord_input_filed.handle_event(&Event::Key(key));
                vec![]
            }
            _ => {
                // max 32 limit as per discord docs
                if self.discord_input_filed.value().chars().count() < 32 {
                    self.discord_input_filed.handle_event(&Event::Key(key));
                }
                vec![]
            }
        };
        send_back
    }
}

impl Component for BetaProgramme {
    fn handle_key_events(&mut self, key: KeyEvent) -> Result<Vec<Action>> {
        if !self.active {
            return Ok(vec![]);
        }
        // while in entry mode, keybinds are not captured, so gotta exit entry mode from here
        let send_back = match &self.state {
            BetaProgrammeState::DiscordIdAlreadySet => self.capture_inputs(key),
            BetaProgrammeState::ShowTCs => {
                match key.code {
                    KeyCode::Char('y') | KeyCode::Char('Y') => {
                        let is_discord_id_set = !self.discord_input_filed.value().is_empty();
                        if is_discord_id_set {
                            debug!("User accepted the TCs, but discord id already set, moving to DiscordIdAlreadySet");
                            self.state = BetaProgrammeState::DiscordIdAlreadySet;
                        } else {
                            debug!("User accepted the TCs, but no discord id set, moving to AcceptTCsAndEnterDiscordId");
                            self.state = BetaProgrammeState::AcceptTCsAndEnterDiscordId;
                        }
                    }
                    KeyCode::Esc => {
                        debug!("User rejected the TCs, moving to RejectTCs");
                        self.state = BetaProgrammeState::RejectTCs;
                    }
                    _ => {}
                }
                vec![]
            }
            BetaProgrammeState::RejectTCs => {
                if let KeyCode::Esc = key.code {
                    debug!("RejectTCs msg closed. Switching to Status scene.");
                    self.state = BetaProgrammeState::ShowTCs;
                }
                vec![Action::SwitchScene(Scene::Status)]
            }
            BetaProgrammeState::AcceptTCsAndEnterDiscordId => self.capture_inputs(key),
        };
        Ok(send_back)
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        let send_back = match action {
            Action::SwitchScene(scene) => match scene {
                Scene::BetaProgrammePopUp => {
                    self.active = true;
                    self.old_value = self.discord_input_filed.value().to_string();
                    // Set to InputMode::Entry as we want to handle everything within our handle_key_events
                    // so by default if this scene is active, we capture inputs.
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

    fn draw(&mut self, f: &mut crate::tui::Frame<'_>, area: Rect) -> Result<()> {
        if !self.active {
            return Ok(());
        }

        let layer_zero = centered_rect_fixed(52, 15, area);

        let layer_one = Layout::new(
            Direction::Vertical,
            [
                // for the pop_up_border
                Constraint::Length(2),
                // for the input field
                Constraint::Min(1),
                // for the pop_up_border
                Constraint::Length(1),
            ],
        )
        .split(layer_zero);

        // layer zero
        let pop_up_border = Paragraph::new("").block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Beta Rewards Program ")
                .title_style(Style::new().fg(VIVID_SKY_BLUE))
                .padding(Padding::uniform(2))
                .border_style(Style::new().fg(VIVID_SKY_BLUE)),
        );
        clear_area(f, layer_zero);

        match self.state {
            BetaProgrammeState::DiscordIdAlreadySet => {
                // split into 4 parts, for the prompt, input, text, dash , and buttons
                let layer_two = Layout::new(
                    Direction::Vertical,
                    [
                        // for the prompt text
                        Constraint::Length(3),
                        // for the input
                        Constraint::Length(3),
                        // for the text
                        Constraint::Length(4),
                        // gap
                        Constraint::Length(1),
                        // for the buttons
                        Constraint::Length(1),
                    ],
                )
                .split(layer_one[1]);

                let prompt_text = Paragraph::new("Discord Username associated with this device:")
                    .alignment(Alignment::Center)
                    .fg(GHOST_WHITE);

                f.render_widget(prompt_text, layer_two[0]);

                let input = Paragraph::new(self.discord_input_filed.value())
                    .alignment(Alignment::Center)
                    .fg(VIVID_SKY_BLUE);
                f.set_cursor(
                    // Put cursor past the end of the input text
                    layer_two[1].x
                        + (layer_two[1].width / 2) as u16
                        + (self.discord_input_filed.value().len() / 2) as u16
                        + if self.discord_input_filed.value().len() % 2 != 0 {
                            1
                        } else {
                            0
                        },
                    layer_two[1].y,
                );
                f.render_widget(input, layer_two[1]);

                let text = Paragraph::new(Text::from(vec![
                    Line::raw("Changing your Username will reset all nodes,"),
                    Line::raw("and any Nanos left on this device will be lost."),
                ]))
                .alignment(Alignment::Center)
                .block(Block::default().padding(Padding::horizontal(2)));

                f.render_widget(text.fg(GHOST_WHITE), layer_two[2]);

                let dash = Block::new()
                    .borders(Borders::BOTTOM)
                    .border_style(Style::new().fg(GHOST_WHITE));
                f.render_widget(dash, layer_two[3]);

                let buttons_layer = Layout::horizontal(vec![
                    Constraint::Percentage(55),
                    Constraint::Percentage(45),
                ])
                .split(layer_two[4]);

                let button_no = Line::from(vec![Span::styled(
                    "  No, Cancel [Esc]",
                    Style::default().fg(LIGHT_PERIWINKLE),
                )]);

                f.render_widget(button_no, buttons_layer[0]);
                let button_yes_style = if self.discord_input_filed.value().is_empty() {
                    Style::default().fg(LIGHT_PERIWINKLE)
                } else {
                    Style::default().fg(EUCALYPTUS)
                };
                let button_yes = Line::from(vec![Span::styled(
                    "Save Username [Enter]",
                    button_yes_style,
                )]);
                f.render_widget(button_yes, buttons_layer[1]);
            }
            BetaProgrammeState::ShowTCs => {
                // split the area into 3 parts, for the lines, hypertext,  buttons
                let layer_two = Layout::new(
                    Direction::Vertical,
                    [
                        // for the text
                        Constraint::Length(6),
                        // for the hypertext
                        Constraint::Length(1),
                        // gap
                        Constraint::Length(5),
                        // for the buttons
                        Constraint::Length(1),
                    ],
                )
                .split(layer_one[1]);

                let text = Paragraph::new("  Earn a slice of millions of tokens created at\n  the genesis of the Autonomi Network by running\n  nodes to build and test the Beta.\n\n  To continue in the beta Rewards Program you\n  agree to the Terms and Conditions found here:");
                f.render_widget(text.fg(GHOST_WHITE), layer_two[0]);
                let link = Hyperlink::new(
                    Span::styled(
                        "  https://autonomi.com/beta/terms",
                        Style::default().fg(VIVID_SKY_BLUE),
                    ),
                    "https://autonomi.com/beta/terms",
                );

                f.render_widget_ref(link, layer_two[1]);

                let dash = Block::new()
                    .borders(Borders::BOTTOM)
                    .border_style(Style::new().fg(GHOST_WHITE));
                f.render_widget(dash, layer_two[2]);

                let buttons_layer = Layout::horizontal(vec![
                    Constraint::Percentage(45),
                    Constraint::Percentage(55),
                ])
                .split(layer_two[3]);

                let button_no = Line::from(vec![Span::styled(
                    "  No, Cancel [Esc]",
                    Style::default().fg(LIGHT_PERIWINKLE),
                )]);
                f.render_widget(button_no, buttons_layer[0]);
                let button_yes = Line::from(vec![Span::styled(
                    "Yes, I agree! Continue [Y]",
                    Style::default().fg(EUCALYPTUS),
                )]);
                f.render_widget(button_yes, buttons_layer[1]);
            }
            BetaProgrammeState::RejectTCs => {
                // split the area into 3 parts, for the lines, hypertext,  buttons
                let layer_two = Layout::new(
                    Direction::Vertical,
                    [
                        // for the text
                        Constraint::Length(7),
                        // gap
                        Constraint::Length(5),
                        // for the buttons
                        Constraint::Length(1),
                    ],
                )
                .split(layer_one[1]);

                let text = Paragraph::new("  Terms and conditions not accepted\n  Beta Rewards Program entry not approved\n  You can still run nodes on the network, but\n  you will not be part of the Beta Rewards\n  Program.\n");
                f.render_widget(text.fg(GHOST_WHITE), layer_two[0]);

                let dash = Block::new()
                    .borders(Borders::BOTTOM)
                    .border_style(Style::new().fg(GHOST_WHITE));
                f.render_widget(dash, layer_two[1]);
                let line = Line::from(vec![Span::styled(
                    "  Close [Esc]",
                    Style::default().fg(LIGHT_PERIWINKLE),
                )]);
                f.render_widget(line, layer_two[2]);
            }
            BetaProgrammeState::AcceptTCsAndEnterDiscordId => {
                // split into 4 parts, for the prompt, input, text, dash , and buttons
                let layer_two = Layout::new(
                    Direction::Vertical,
                    [
                        // for the prompt text
                        Constraint::Length(3),
                        // for the input
                        Constraint::Length(2),
                        // for the text
                        Constraint::Length(3),
                        // gap
                        Constraint::Length(3),
                        // for the buttons
                        Constraint::Length(1),
                    ],
                )
                .split(layer_one[1]);

                let prompt =
                    Paragraph::new("Enter your Discord Username").alignment(Alignment::Center);

                f.render_widget(prompt.fg(GHOST_WHITE), layer_two[0]);

                let input = Paragraph::new(self.discord_input_filed.value())
                    .alignment(Alignment::Center)
                    .fg(VIVID_SKY_BLUE);
                f.set_cursor(
                    // Put cursor past the end of the input text
                    layer_two[1].x
                        + (layer_two[1].width / 2) as u16
                        + (self.discord_input_filed.value().len() / 2) as u16
                        + if self.discord_input_filed.value().len() % 2 != 0 {
                            1
                        } else {
                            0
                        },
                    layer_two[1].y,
                );
                f.render_widget(input, layer_two[1]);

                let text = Paragraph::new("  Submit your username and track your progress on\n  our Discord server. Note: your username may be\n  different from your display name.");
                f.render_widget(text.fg(GHOST_WHITE), layer_two[2]);

                let dash = Block::new()
                    .borders(Borders::BOTTOM)
                    .border_style(Style::new().fg(GHOST_WHITE));
                f.render_widget(dash, layer_two[3]);

                let buttons_layer = Layout::horizontal(vec![
                    Constraint::Percentage(50),
                    Constraint::Percentage(50),
                ])
                .split(layer_two[4]);

                let button_no = Line::from(vec![Span::styled(
                    "  No, Cancel [Esc]",
                    Style::default().fg(LIGHT_PERIWINKLE),
                )]);
                let button_yes_style = if self.discord_input_filed.value().is_empty() {
                    Style::default().fg(LIGHT_PERIWINKLE)
                } else {
                    Style::default().fg(EUCALYPTUS)
                };
                f.render_widget(button_no, buttons_layer[0]);
                let button_yes = Line::from(vec![Span::styled(
                    "Submit Username [Enter]",
                    button_yes_style,
                )]);
                f.render_widget(button_yes, buttons_layer[1]);
            }
        }

        f.render_widget(pop_up_border, layer_zero);

        Ok(())
    }
}
