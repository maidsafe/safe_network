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
};

use super::{utils::centered_rect_fixed, Component};

const GB_PER_NODE: usize = 5;

pub struct ResourceAllocationInputBox {
    show_scene: bool,
    available_disk_space: usize,
    allocated_space_input: Input,
    // cache the old value incase user presses Esc.
    old_value: String,
}

impl ResourceAllocationInputBox {
    pub fn new(allocated_space: usize) -> Result<Self> {
        let new = Self {
            show_scene: false,
            available_disk_space: Self::get_available_space_gb()?,
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
        let available_space_gb = available_space_b / (1024 * 1024 * 1024);

        Ok(available_space_gb)
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

impl Component for ResourceAllocationInputBox {
    fn handle_key_events(&mut self, key: KeyEvent) -> Result<Vec<Action>> {
        // while in entry mode, keybinds are not captured, so gotta exit entry mode from here
        let send_back = match key.code {
            KeyCode::Enter => {
                let allocated_space_str = self.allocated_space_input.value().to_string();
                let allocated_space = if let Ok(allocated_space) =
                    allocated_space_str.trim().parse::<usize>()
                {
                    let allocated_space = std::cmp::min(allocated_space, self.available_disk_space);
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
            _ => {
                vec![]
            }
        };
        Ok(send_back)
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        let send_back = match action {
            Action::SwitchScene(scene) => match scene {
                Scene::ResourceAllocationInputBox => {
                    self.show_scene = true;
                    self.old_value = self.allocated_space_input.value().to_string();
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

        let layer_zero = centered_rect_fixed(40, 5, area);

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
                .border_type(BorderType::Double)
                .border_style(Style::new().bold())
                .title(format!(
                    " Allocate space ({} GB available)",
                    self.available_disk_space
                )),
        );
        f.render_widget(Clear, layer_zero);
        f.render_widget(pop_up_border, layer_zero);

        // layer one - 1
        let width = layer_one[1].width.max(3) - 3;

        let scroll = self.allocated_space_input.visual_scroll(width as usize);
        let input = Paragraph::new(self.allocated_space_input.value())
            .scroll((0, scroll as u16))
            // border left so that it doesn't conflict with layer_zero's border
            .block(
                Block::default()
                    .borders(Borders::LEFT)
                    .border_type(BorderType::Double)
                    .border_style(Style::new().bold()),
            );

        f.set_cursor(
            // Put cursor past the end of the input text
            layer_one[1].x
                + ((self.allocated_space_input.visual_cursor()).max(scroll) - scroll) as u16
                + 1,
            layer_one[1].y,
        );
        f.render_widget(input, layer_one[1]);

        Ok(())
    }
}
