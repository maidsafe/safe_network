// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use color_eyre::{eyre::ContextCompat, Result};
use crossterm::event::{Event, KeyCode, KeyEvent};
use ratatui::{prelude::*, widgets::*};
use std::path::PathBuf;
use sysinfo::Disks;
use tui_input::{backend::crossterm::EventHandler, Input};

use crate::{
    action::Action,
    mode::{InputMode, Scene},
    style::{EUCALYPTUS, GHOST_WHITE, LIGHT_PERIWINKLE, VIVID_SKY_BLUE},
};

use super::{utils::centered_rect_fixed, Component};

pub const GB_PER_NODE: usize = 5;
pub const MB: usize = 1000 * 1000;
pub const GB: usize = MB * 1000;

pub struct ManageNodes {
    /// Whether the component is active right now, capturing keystrokes + drawing things.
    active: bool,
    available_disk_space_bytes: usize,
    allocated_space_input: Input,
    // cache the old value incase user presses Esc.
    old_value: String,
}

impl ManageNodes {
    pub fn new(allocated_space: usize) -> Result<Self> {
        let new = Self {
            active: false,
            available_disk_space_bytes: Self::get_available_space_gb()?,
            allocated_space_input: Input::default().with_value(allocated_space.to_string()),
            old_value: Default::default(),
        };
        Ok(new)
    }

    fn get_available_space_gb() -> Result<usize> {
        let disks = Disks::new_with_refreshed_list();

        let available_space_b = disks
            .list()
            .iter()
            .find(|disk| disk.mount_point().starts_with(Self::get_mount_point()))
            .context("Cannot find the primary disk")?
            .available_space() as usize;

        Ok(available_space_b)
    }

    #[cfg(unix)]
    fn get_mount_point() -> PathBuf {
        PathBuf::from("/")
    }
    #[cfg(windows)]
    fn get_mount_point() -> PathBuf {
        PathBuf::from("C:\\")
    }
}

impl Component for ManageNodes {
    fn handle_key_events(&mut self, key: KeyEvent) -> Result<Vec<Action>> {
        if !self.active {
            return Ok(vec![]);
        }

        // while in entry mode, key bindings are not captured, so gotta exit entry mode from here
        let send_back = match key.code {
            KeyCode::Enter => {
                let allocated_space_str = self.allocated_space_input.value().to_string();
                let allocated_space =
                    if let Ok(allocated_space) = allocated_space_str.trim().parse::<usize>() {
                        let allocated_space =
                            std::cmp::min(allocated_space, self.available_disk_space_bytes);
                        let max_nodes = allocated_space / GB_PER_NODE;
                        max_nodes * GB_PER_NODE
                    } else {
                        0
                    };
                // set the new value
                self.allocated_space_input = self
                    .allocated_space_input
                    .clone()
                    .with_value(allocated_space.to_string());

                debug!(
                    "Got Enter, value found to be {allocated_space} derived from input: {allocated_space_str:?} and switching scene",
                );
                vec![
                    Action::StoreAllocatedDiskSpace(allocated_space),
                    Action::SwitchScene(Scene::Home),
                ]
            }
            KeyCode::Esc => {
                debug!(
                    "Got Esc, restoring the old value {} and switching to home",
                    self.old_value
                );
                // reset to old value
                self.allocated_space_input = self
                    .allocated_space_input
                    .clone()
                    .with_value(self.old_value.clone());
                vec![Action::SwitchScene(Scene::Home)]
            }
            KeyCode::Char(c) if c.is_numeric() => {
                self.allocated_space_input.handle_event(&Event::Key(key));
                vec![]
            }
            KeyCode::Backspace => {
                self.allocated_space_input.handle_event(&Event::Key(key));
                vec![]
            }
            KeyCode::Up | KeyCode::Down => {
                let allocated_space_str = self.allocated_space_input.value().to_string();
                let allocated_space = if let Ok(allocated_space) =
                    allocated_space_str.trim().parse::<usize>()
                {
                    if key.code == KeyCode::Up {
                        if allocated_space + GB_PER_NODE <= self.available_disk_space_bytes / GB {
                            allocated_space + GB_PER_NODE
                        } else {
                            allocated_space
                        }
                    } else {
                        // Key::Down
                        if allocated_space >= GB_PER_NODE {
                            allocated_space - GB_PER_NODE
                        } else {
                            allocated_space
                        }
                    }
                } else {
                    0
                };
                // set the new value
                self.allocated_space_input = self
                    .allocated_space_input
                    .clone()
                    .with_value(allocated_space.to_string());
                vec![]
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
                Scene::ManageNodes => {
                    self.active = true;
                    self.old_value = self.allocated_space_input.value().to_string();
                    // set to entry input mode as we want to handle everything within our handle_key_events
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
                Constraint::Length(1),
                // for the info field telling how much gb used
                Constraint::Length(1),
                // gap before help
                Constraint::Length(1),
                // for the help
                Constraint::Length(3),
                // for the dash
                Constraint::Min(1),
                // for the buttons
                Constraint::Length(1),
                // for the pop_up_border
                Constraint::Length(1),
            ],
        )
        .split(layer_zero);
        let pop_up_border = Paragraph::new("").block(
            Block::default()
                .borders(Borders::ALL)
                .title("Manage Nodes")
                .padding(Padding::uniform(2))
                .border_style(Style::new().fg(GHOST_WHITE)),
        );
        f.render_widget(Clear, layer_zero);

        // ==== input field ====
        let layer_input_field = Layout::new(
            Direction::Horizontal,
            [
                // for the gap
                Constraint::Min(5),
                // Start
                Constraint::Length(5),
                // Input box
                Constraint::Length(5),
                // Nodes(s)
                Constraint::Length(8),
                // gap
                Constraint::Min(5),
            ],
        )
        .split(layer_one[1]);

        let start = Paragraph::new("Start").style(Style::default().fg(GHOST_WHITE));
        f.render_widget(start, layer_input_field[1]);

        let width = layer_input_field[2].width.max(3) - 3;
        let scroll = self.allocated_space_input.visual_scroll(width as usize);
        let input = Paragraph::new(self.allocated_space_input.value())
            .style(Style::new().fg(VIVID_SKY_BLUE))
            .scroll((0, scroll as u16))
            .alignment(Alignment::Center);

        f.render_widget(input, layer_input_field[2]);

        let nodes_text = Paragraph::new("Node(s)").style(Style::default().fg(GHOST_WHITE));
        f.render_widget(nodes_text, layer_input_field[3]);

        // ==== info field ====
        let available_space_gb = self.available_disk_space_bytes / GB;
        let info_style = Style::default().fg(VIVID_SKY_BLUE);
        let info = Line::from(vec![
            Span::styled("Using", info_style),
            Span::styled(
                format!(" {}GB ", self.allocated_space_input.value()),
                info_style.bold(),
            ),
            Span::styled(
                format!("of {available_space_gb}GB available space"),
                info_style,
            ),
        ]);
        let info = Paragraph::new(info).alignment(Alignment::Center);
        f.render_widget(info, layer_one[2]);

        // ==== help ====
        let help = Paragraph::new("  Note: Each node will use a small amount of CPU\n  Memory and Network Bandwidth. We recommend\n  starting no more than 5 at a time.")
            .style(Style::default().fg(GHOST_WHITE));
        f.render_widget(help, layer_one[4]);

        // ==== dash ====
        let dash = Block::new()
            .borders(Borders::BOTTOM)
            .border_style(Style::new().fg(GHOST_WHITE));
        f.render_widget(dash, layer_one[5]);

        // ==== buttons ====
        let buttons_layer =
            Layout::horizontal(vec![Constraint::Percentage(45), Constraint::Percentage(55)])
                .split(layer_one[6]);

        let button_no = Line::from(vec![Span::styled(
            "  Close [Esc]",
            Style::default().fg(LIGHT_PERIWINKLE),
        )]);
        f.render_widget(button_no, buttons_layer[0]);
        let button_yes = Line::from(vec![Span::styled(
            "Start Node(s) [Enter]  ",
            Style::default().fg(EUCALYPTUS),
        )]);
        let button_yes = Paragraph::new(button_yes).alignment(Alignment::Right);
        f.render_widget(button_yes, buttons_layer[1]);

        f.render_widget(pop_up_border, layer_zero);

        Ok(())
    }
}
