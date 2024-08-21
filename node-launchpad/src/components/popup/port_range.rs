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
    connection_mode::ConnectionMode,
    mode::{InputMode, Scene},
    style::{clear_area, EUCALYPTUS, GHOST_WHITE, INDIGO, LIGHT_PERIWINKLE, VIVID_SKY_BLUE},
};
use color_eyre::Result;
use crossterm::event::{Event, KeyCode, KeyEvent};
use ratatui::{prelude::*, widgets::*};
use tui_input::{backend::crossterm::EventHandler, Input};

const PORT_MAX: u16 = 65535;
const PORT_MIN: u16 = 1024;
const INPUT_SIZE: u16 = 5;
const INPUT_AREA: u16 = INPUT_SIZE + 2; // +2 for the left and right padding

#[derive(PartialEq)]
enum FocusInput {
    PortFrom,
    PortTo,
}

pub struct PortRangePopUp {
    active: bool,
    connection_mode: ConnectionMode,
    port_from: Input,
    port_to: Input,
    port_from_old_value: u16,
    port_to_old_value: u16,
    focus: FocusInput,
    can_save: bool,
}

impl PortRangePopUp {
    pub fn new(connection_mode: ConnectionMode, port_from: u16, port_to: u16) -> Self {
        Self {
            active: false,
            connection_mode,
            port_from: Input::default().with_value(port_from.to_string()),
            port_to: Input::default().with_value(port_to.to_string()),
            port_from_old_value: Default::default(),
            port_to_old_value: Default::default(),
            focus: FocusInput::PortFrom,
            can_save: false,
        }
    }

    pub fn validate(&mut self) {
        if self.port_from.value().is_empty() || self.port_to.value().is_empty() {
            self.can_save = false;
        } else {
            let port_from: u16 = self.port_from.value().parse().unwrap_or_default();
            let port_to: u16 = self.port_to.value().parse().unwrap_or_default();
            self.can_save = (PORT_MIN..=PORT_MAX).contains(&port_from)
                && (PORT_MIN..=PORT_MAX).contains(&port_to)
                && port_from <= port_to;
        }
    }
}

impl Component for PortRangePopUp {
    fn handle_key_events(&mut self, key: KeyEvent) -> Result<Vec<Action>> {
        if !self.active {
            return Ok(vec![]);
        }
        // while in entry mode, keybinds are not captured, so gotta exit entry mode from here
        let send_back = match key.code {
            KeyCode::Enter => {
                let port_from = self.port_from.value();
                let port_to = self.port_to.value();

                if port_from.is_empty() || port_to.is_empty() || !self.can_save {
                    debug!("Got Enter, but port_from or port_to is empty, ignoring.");
                    return Ok(vec![]);
                }
                debug!("Got Enter, saving the ports and switching to Options Screen",);
                vec![
                    Action::StorePortRange(
                        self.port_from.value().parse().unwrap_or_default(),
                        self.port_to.value().parse().unwrap_or_default(),
                    ),
                    Action::OptionsActions(OptionsActions::UpdatePortRange(
                        self.port_from.value().parse().unwrap_or_default(),
                        self.port_to.value().parse().unwrap_or_default(),
                    )),
                    Action::SwitchScene(Scene::Options),
                ]
            }
            KeyCode::Esc => {
                debug!("Got Esc, restoring the old values and switching to actual screen");
                // reset to old value
                self.port_from = self
                    .port_from
                    .clone()
                    .with_value(self.port_from_old_value.to_string());
                self.port_to = self
                    .port_to
                    .clone()
                    .with_value(self.port_to_old_value.to_string());
                vec![Action::SwitchScene(Scene::Options)]
            }
            KeyCode::Char(c) if !c.is_numeric() => vec![],
            KeyCode::Tab => {
                self.focus = if self.focus == FocusInput::PortFrom {
                    FocusInput::PortTo
                } else {
                    FocusInput::PortFrom
                };
                vec![]
            }
            KeyCode::Up => {
                if self.focus == FocusInput::PortFrom
                    && self.port_from.value().parse::<u16>().unwrap_or_default() < PORT_MAX
                {
                    self.port_from = self.port_from.clone().with_value(
                        (self.port_from.value().parse::<u16>().unwrap_or_default() + 1).to_string(),
                    );
                } else if self.focus == FocusInput::PortTo
                    && self.port_from.value().parse::<u16>().unwrap_or_default() > PORT_MIN
                {
                    self.port_to = self.port_to.clone().with_value(
                        (self.port_to.value().parse::<u16>().unwrap_or_default() + 1).to_string(),
                    );
                }
                self.validate();
                vec![]
            }
            KeyCode::Down => {
                if self.focus == FocusInput::PortFrom
                    && self.port_from.value().parse::<u16>().unwrap_or_default() > PORT_MIN
                {
                    self.port_from = self.port_from.clone().with_value(
                        (self.port_from.value().parse::<u16>().unwrap_or_default() - 1).to_string(),
                    );
                } else if self.focus == FocusInput::PortTo
                    && self.port_to.value().parse::<u16>().unwrap_or_default() < PORT_MAX
                {
                    self.port_to = self.port_to.clone().with_value(
                        (self.port_to.value().parse::<u16>().unwrap_or_default() - 1).to_string(),
                    );
                }
                self.validate();
                vec![]
            }
            KeyCode::Backspace => {
                // if max limit reached, we should allow Backspace to work.
                if self.focus == FocusInput::PortFrom {
                    self.port_from.handle_event(&Event::Key(key));
                } else if self.focus == FocusInput::PortTo {
                    self.port_to.handle_event(&Event::Key(key));
                }
                self.validate();
                vec![]
            }
            _ => {
                // if max limit reached, we should not allow any more inputs.
                if self.focus == FocusInput::PortFrom
                    && self.port_from.value().len() < INPUT_SIZE as usize
                {
                    self.port_from.handle_event(&Event::Key(key));
                } else if self.focus == FocusInput::PortTo
                    && self.port_to.value().len() < INPUT_SIZE as usize
                {
                    self.port_to.handle_event(&Event::Key(key));
                }

                self.validate();
                vec![]
            }
        };
        Ok(send_back)
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        let send_back = match action {
            Action::SwitchScene(scene) => match scene {
                Scene::ChangePortsPopUp => {
                    if self.connection_mode == ConnectionMode::CustomPorts {
                        self.active = true;
                        self.validate();
                        self.port_from_old_value =
                            self.port_from.value().parse().unwrap_or_default();
                        self.port_to_old_value = self.port_to.value().parse().unwrap_or_default();
                        // Set to InputMode::Entry as we want to handle everything within our handle_key_events
                        // so by default if this scene is active, we capture inputs.
                        Some(Action::SwitchInputMode(InputMode::Entry))
                    } else {
                        self.active = false;
                        Some(Action::SwitchScene(Scene::Options))
                    }
                }
                _ => {
                    self.active = false;
                    None
                }
            },
            // Useful when the user has selected a connection mode but didn't confirm it
            Action::OptionsActions(OptionsActions::UpdateConnectionMode(connection_mode)) => {
                self.connection_mode = connection_mode;
                None
            }
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
                .title(" Custom Ports ")
                .title_style(Style::new().fg(VIVID_SKY_BLUE))
                .padding(Padding::uniform(2))
                .border_style(Style::new().fg(VIVID_SKY_BLUE)),
        );
        clear_area(f, layer_zero);

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

        let prompt = Paragraph::new("Enter Port Number")
            .bold()
            .alignment(Alignment::Center);

        f.render_widget(prompt.fg(GHOST_WHITE), layer_two[0]);

        let spaces_from = " ".repeat((INPUT_AREA - 1) as usize - self.port_from.value().len());
        let spaces_to = " ".repeat((INPUT_AREA - 1) as usize - self.port_to.value().len());

        let input_line = Line::from(vec![
            Span::styled(
                format!("{}{} ", spaces_from, self.port_from.value()),
                if self.focus == FocusInput::PortFrom {
                    Style::default()
                        .fg(VIVID_SKY_BLUE)
                        .bg(INDIGO)
                        .underlined()
                        .underline_color(VIVID_SKY_BLUE)
                } else {
                    Style::default().fg(VIVID_SKY_BLUE)
                },
            ),
            Span::styled(" to ", Style::default().fg(GHOST_WHITE)),
            Span::styled(
                format!("{}{} ", spaces_to, self.port_to.value()),
                if self.focus == FocusInput::PortTo {
                    Style::default()
                        .fg(VIVID_SKY_BLUE)
                        .bg(INDIGO)
                        .underlined()
                        .underline_color(VIVID_SKY_BLUE)
                } else {
                    Style::default().fg(VIVID_SKY_BLUE)
                },
            ),
        ])
        .alignment(Alignment::Center);

        f.render_widget(input_line, layer_two[1]);

        let text = Paragraph::new("Choose the start of the port range. The range will then match the number of nodes on this device.")
            .block(block::Block::default().padding(Padding::horizontal(2)))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });
        f.render_widget(text.fg(GHOST_WHITE), layer_two[2]);

        let dash = Block::new()
            .borders(Borders::BOTTOM)
            .border_style(Style::new().fg(GHOST_WHITE));
        f.render_widget(dash, layer_two[3]);

        let buttons_layer =
            Layout::horizontal(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(layer_two[4]);

        let button_no = Line::from(vec![Span::styled(
            "  Cancel [Esc]",
            Style::default().fg(LIGHT_PERIWINKLE),
        )]);
        let button_yes_style = if self.can_save {
            Style::default().fg(EUCALYPTUS)
        } else {
            Style::default().fg(LIGHT_PERIWINKLE)
        };
        f.render_widget(button_no, buttons_layer[0]);
        let button_yes = Line::from(vec![Span::styled(
            "Save Port Range [Enter]",
            button_yes_style,
        )]);
        f.render_widget(button_yes, buttons_layer[1]);

        f.render_widget(pop_up_border, layer_zero);

        Ok(())
    }
}
