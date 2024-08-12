// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::super::utils::centered_rect_fixed;

use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Padding, Paragraph, Wrap},
};

use crate::{
    action::{Action, OptionsActions},
    components::Component,
    mode::{InputMode, Scene},
    style::{clear_area, DARK_GUNMETAL, EUCALYPTUS, GHOST_WHITE, LIGHT_PERIWINKLE, VIVID_SKY_BLUE},
};

#[derive(Default)]
pub struct ChangeDriveConfirmPopup {
    active: bool,
    drive_selection_mountpoint: String,
    drive_selection_name: String,
}

impl Component for ChangeDriveConfirmPopup {
    fn handle_key_events(&mut self, key: KeyEvent) -> Result<Vec<Action>> {
        if !self.active {
            return Ok(vec![]);
        }
        let send_back = match key.code {
            KeyCode::Enter => {
                debug!("Got Enter, storing value and switching to Options");
                vec![
                    Action::StoreStorageDrive(
                        self.drive_selection_mountpoint.clone(),
                        self.drive_selection_name.clone(),
                    ),
                    Action::OptionsActions(OptionsActions::UpdateStorageDrive(
                        self.drive_selection_mountpoint.clone(),
                        self.drive_selection_name.clone(),
                    )),
                    Action::SwitchScene(Scene::Options),
                ]
            }
            KeyCode::Esc => {
                debug!("Got Esc, switching to Options");
                vec![Action::SwitchScene(Scene::Options)]
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
                Scene::ChangeDriveConfirmPopup => {
                    self.active = true;
                    Some(Action::SwitchInputMode(InputMode::Entry))
                }
                _ => {
                    self.active = false;
                    None
                }
            },
            Action::OptionsActions(OptionsActions::TriggerChangeDriveConfirm(mountpoint, name)) => {
                self.drive_selection_mountpoint = mountpoint;
                self.drive_selection_name = name;
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
                // Text
                Constraint::Min(1),
                // for the pop_up_border
                Constraint::Length(1),
            ],
        )
        .split(layer_zero);

        let pop_up_border = Paragraph::new("").block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Confirm & Reset ")
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

        // Text
        let text = vec![
            Line::from(vec![]), // Empty line
            Line::from(vec![]), // Empty line
            Line::from(vec![
                Span::styled("Changing storage to ", Style::default().fg(GHOST_WHITE)),
                Span::styled(
                    format!("{} ", self.drive_selection_name),
                    Style::default().fg(VIVID_SKY_BLUE),
                ),
                Span::styled("will ", Style::default().fg(GHOST_WHITE)),
            ])
            .alignment(Alignment::Center),
            Line::from(vec![Span::styled(
                "reset all nodes.",
                Style::default().fg(GHOST_WHITE),
            )])
            .alignment(Alignment::Center),
            Line::from(vec![]), // Empty line
            Line::from(vec![]), // Empty line
            Line::from(vec![
                Span::styled("You’ll need to ", Style::default().fg(GHOST_WHITE)),
                Span::styled("Add ", Style::default().fg(GHOST_WHITE).bold()),
                Span::styled("and ", Style::default().fg(GHOST_WHITE)),
                Span::styled("Start ", Style::default().fg(GHOST_WHITE).bold()),
                Span::styled(
                    "them again afterwards. Are you sure you want to continue?",
                    Style::default().fg(GHOST_WHITE),
                ),
            ])
            .alignment(Alignment::Center),
        ];
        let paragraph = Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .block(
                Block::default()
                    .borders(Borders::NONE)
                    .padding(Padding::horizontal(2)),
            )
            .alignment(Alignment::Center)
            .style(Style::default().fg(GHOST_WHITE).bg(DARK_GUNMETAL));

        f.render_widget(paragraph, layer_two[0]);

        // Dash
        let dash = Block::new()
            .borders(Borders::BOTTOM)
            .border_style(Style::new().fg(GHOST_WHITE));
        f.render_widget(dash, layer_two[1]);

        // Buttons
        let buttons_layer =
            Layout::horizontal(vec![Constraint::Percentage(30), Constraint::Percentage(70)])
                .split(layer_two[2]);

        let button_no = Line::from(vec![Span::styled(
            "Back [Esc]",
            Style::default().fg(LIGHT_PERIWINKLE),
        )]);

        f.render_widget(
            Paragraph::new(button_no)
                .block(Block::default().padding(Padding::horizontal(2)))
                .alignment(Alignment::Left),
            buttons_layer[0],
        );

        let button_yes = Line::from(vec![
            Span::styled("Yes, change drive ", Style::default().fg(EUCALYPTUS)),
            Span::styled("[Enter]", Style::default().fg(LIGHT_PERIWINKLE).bold()),
        ])
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
