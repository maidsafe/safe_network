// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use std::default::Default;

use super::super::utils::centered_rect_fixed;

use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, HighlightSpacing, List, ListItem, ListState, Padding, Paragraph},
};

use crate::{
    action::{Action, OptionsActions},
    components::Component,
    mode::{InputMode, Scene},
    style::{
        clear_area, COOL_GREY, DARK_GUNMETAL, EUCALYPTUS, GHOST_WHITE, INDIGO, LIGHT_PERIWINKLE,
        SPACE_CADET, VIVID_SKY_BLUE,
    },
    system,
};

#[derive(Default)]
pub struct ChangeDrivePopup {
    active: bool,
    items: StatefulList<DriveItem>,
    drive_selection: DriveItem,
    user_moved: bool, // This is used to check if the user has moved the selection and style it accordingly
}

impl ChangeDrivePopup {
    pub fn new(storage_mountpoint: String) -> Self {
        let drives_and_space = system::get_list_of_drives_and_available_space();

        let mut selected_drive: DriveItem = DriveItem::default();
        // Create a vector of DriveItem from drives_and_space
        let drives_items: Vec<DriveItem> = drives_and_space
            .iter()
            .map(|(drive_name, mountpoint, space)| {
                let size_str = format!("{:.2} GB", *space as f64 / 1e9);
                let size_str_cloned = size_str.clone();
                DriveItem {
                    name: drive_name.to_string(),
                    mountpoint: mountpoint.to_string(),
                    size: size_str,
                    status: if mountpoint == &storage_mountpoint {
                        selected_drive = DriveItem {
                            name: drive_name.to_string(),
                            mountpoint: mountpoint.to_string(),
                            size: size_str_cloned,
                            status: DriveStatus::Selected,
                        };
                        DriveStatus::Selected
                    } else {
                        DriveStatus::NotSelected
                    },
                }
            })
            .collect::<Vec<DriveItem>>();
        debug!("Drive Mountpoint in Config: {}", storage_mountpoint);
        debug!("Drives and space: {:?}", drives_and_space);
        let items = StatefulList::with_items(drives_items);
        Self {
            active: false,
            items,
            drive_selection: selected_drive,
            user_moved: false,
        }
    }
    // Deselect all drives
    fn deselect_all(&mut self) {
        for item in &mut self.items.items {
            item.status = DriveStatus::NotSelected;
        }
    }
    // Change the status of the selected drive to Selected
    #[allow(dead_code)]
    fn change_status(&mut self) {
        self.deselect_all();
        if let Some(i) = self.items.state.selected() {
            self.items.items[i].status = DriveStatus::Selected;
            self.drive_selection = self.items.items[i].clone();
        }
    }
    // Highlight the drive that is currently selected in this component members
    fn select_drive(&mut self) {
        self.deselect_all();
        for (index, item) in self.items.items.iter_mut().enumerate() {
            if item.mountpoint == self.drive_selection.mountpoint {
                item.status = DriveStatus::Selected;
                self.items.state.select(Some(index));
                break;
            }
        }
    }
    // return the selected drive
    fn return_selection(&mut self) -> DriveItem {
        if let Some(i) = self.items.state.selected() {
            return self.items.items[i].clone();
        }
        DriveItem::default()
    }
}

impl Component for ChangeDrivePopup {
    fn handle_key_events(&mut self, key: KeyEvent) -> Result<Vec<Action>> {
        if !self.active {
            return Ok(vec![]);
        }
        let send_back = match key.code {
            KeyCode::Enter => {
                // We allow action if we have more than one drive and the action is not
                // over the drive already selected
                let drive = self.return_selection();
                if self.items.items.len() > 1
                    && (drive.name != self.drive_selection.name
                        && drive.mountpoint != self.drive_selection.mountpoint)
                {
                    debug!(
                        "Got Enter and there's a new selection, storing value and switching to Options"
                    );
                    // self.change_status();
                    debug!("Drive selected: {:?}", self.drive_selection.name);
                    vec![
                        Action::OptionsActions(OptionsActions::TriggerChangeDriveConfirm(
                            drive.mountpoint.clone(),
                            drive.name.clone(),
                        )),
                        Action::SwitchScene(Scene::ChangeDriveConfirmPopup),
                    ]
                } else {
                    debug!("Got Enter, but no new selection. We should not do anything");
                    vec![Action::SwitchScene(Scene::ChangeDrivePopUp)]
                }
            }
            KeyCode::Esc => {
                debug!("Got Esc, switching to Options");
                vec![Action::SwitchScene(Scene::Options)]
            }
            KeyCode::Up => {
                let drive = self.return_selection();
                if self.items.items.len() > 1 {
                    self.user_moved = drive.name == self.drive_selection.name
                        && drive.mountpoint == self.drive_selection.mountpoint;
                    self.items.previous();
                }
                vec![]
            }
            KeyCode::Down => {
                let drive = self.return_selection();
                if self.items.items.len() > 1 {
                    self.user_moved = drive.name == self.drive_selection.name
                        && drive.mountpoint == self.drive_selection.mountpoint;
                    self.items.next();
                }
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
                Scene::ChangeDrivePopUp => {
                    self.active = true;
                    self.user_moved = false;
                    self.select_drive();
                    Some(Action::SwitchInputMode(InputMode::Entry))
                }
                _ => {
                    self.active = false;
                    None
                }
            },
            // Useful when the user has selected a drive but didn't confirm it
            Action::OptionsActions(OptionsActions::UpdateStorageDrive(mountpoint, drive_name)) => {
                self.drive_selection.mountpoint = mountpoint;
                self.drive_selection.name = drive_name;
                self.select_drive();
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
                // Padding from title to the table
                Constraint::Length(1),
                // Table
                Constraint::Min(1),
                // for the pop_up_border
                Constraint::Length(1),
            ],
        )
        .split(layer_zero);

        let pop_up_border = Paragraph::new("").block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Select a Drive ")
                .title_style(Style::new().fg(VIVID_SKY_BLUE))
                .padding(Padding::uniform(2))
                .border_style(Style::new().fg(VIVID_SKY_BLUE))
                .bg(DARK_GUNMETAL),
        );
        clear_area(f, layer_zero);

        let layer_two = Layout::new(
            Direction::Vertical,
            [
                // for the table
                Constraint::Length(10),
                // gap
                Constraint::Length(3),
                // for the buttons
                Constraint::Length(1),
            ],
        )
        .split(layer_one[1]);

        // Drive selector
        let items: Vec<ListItem> = self
            .items
            .items
            .iter()
            .enumerate()
            .map(|(i, drive_item)| drive_item.to_list_item(i, layer_two[0].width as usize))
            .collect();

        let items = List::new(items)
            .block(Block::default().padding(Padding::uniform(1)))
            .highlight_style(
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .add_modifier(Modifier::REVERSED)
                    .fg(INDIGO),
            )
            .highlight_spacing(HighlightSpacing::Always);

        f.render_stateful_widget(items, layer_two[0], &mut self.items.state);

        // Dash
        let dash = Block::new()
            .borders(Borders::BOTTOM)
            .border_style(Style::new().fg(GHOST_WHITE));
        f.render_widget(dash, layer_two[1]);

        // Buttons
        let buttons_layer =
            Layout::horizontal(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(layer_two[2]);

        let button_no = Line::from(vec![Span::styled(
            "Cancel [Esc]",
            Style::default().fg(LIGHT_PERIWINKLE),
        )]);

        f.render_widget(
            Paragraph::new(button_no)
                .block(Block::default().padding(Padding::horizontal(2)))
                .alignment(Alignment::Left),
            buttons_layer[0],
        );

        let button_yes = Line::from(if self.user_moved {
            vec![
                Span::styled("Change Drive ", Style::default().fg(EUCALYPTUS)),
                Span::styled("[Enter]", Style::default().fg(LIGHT_PERIWINKLE).bold()),
            ]
        } else {
            vec![
                Span::styled("Change Drive ", Style::default().fg(COOL_GREY)),
                Span::styled("[Enter]", Style::default().fg(LIGHT_PERIWINKLE)),
            ]
        })
        .alignment(Alignment::Right);

        f.render_widget(
            Paragraph::new(button_yes)
                .block(Block::default().padding(Padding::horizontal(2)))
                .alignment(Alignment::Right),
            buttons_layer[1],
        );

        // We render now so the borders are on top of the other widgets
        f.render_widget(pop_up_border, layer_zero);

        Ok(())
    }
}

#[derive(Default)]
struct StatefulList<T> {
    state: ListState,
    items: Vec<T>,
    last_selected: Option<usize>,
}

impl<T> StatefulList<T> {
    fn with_items(items: Vec<T>) -> Self {
        StatefulList {
            state: ListState::default(),
            items,
            last_selected: None,
        }
    }

    fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.items.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => self.last_selected.unwrap_or(0),
        };
        self.state.select(Some(i));
    }

    fn previous(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.items.len() - 1
                } else {
                    i - 1
                }
            }
            None => self.last_selected.unwrap_or(0),
        };
        self.state.select(Some(i));
    }
}

#[derive(Default, Debug, Copy, Clone)]
enum DriveStatus {
    Selected,
    #[default]
    NotSelected,
}

#[derive(Default, Debug, Clone)]
pub struct DriveItem {
    name: String,
    mountpoint: String,
    size: String,
    status: DriveStatus,
}

impl DriveItem {
    fn to_list_item(&self, _index: usize, width: usize) -> ListItem {
        let spaces = width - self.name.len() - self.size.len() - "   ".len() - 4;

        let line = match self.status {
            DriveStatus::NotSelected => Line::from(vec![
                Span::raw("   "),
                Span::styled(self.name.clone(), Style::default().fg(VIVID_SKY_BLUE)),
                Span::raw(" ".repeat(spaces)),
                Span::styled(self.size.clone(), Style::default().fg(LIGHT_PERIWINKLE)),
            ]),
            DriveStatus::Selected => Line::from(vec![
                Span::styled(" ►", Style::default().fg(EUCALYPTUS)),
                Span::raw(" "),
                Span::styled(self.name.clone(), Style::default().fg(VIVID_SKY_BLUE)),
                Span::raw(" ".repeat(spaces)),
                Span::styled(self.size.clone(), Style::default().fg(GHOST_WHITE)),
            ]),
        };

        ListItem::new(line).style(Style::default().bg(SPACE_CADET))
    }
}
