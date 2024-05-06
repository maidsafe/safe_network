// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::Component;
use crate::{action::Action, mode::Scene};
use color_eyre::eyre::Result;
use ratatui::{prelude::*, widgets::*};

#[derive(Default)]
pub struct Footer {
    current_scene: Scene,
}

impl Component for Footer {
    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        if let Action::SwitchScene(scene) = action {
            self.current_scene = scene;
        }
        Ok(None)
    }

    fn draw(&mut self, f: &mut crate::tui::Frame<'_>, area: Rect) -> Result<()> {
        let layer_zero = Layout::new(
            Direction::Vertical,
            [Constraint::Min(1), Constraint::Max(3)],
        )
        .split(area);

        let text = match self.current_scene {
            Scene::Home => {
                "[A]dd node, [S]tart node, [K]ill node, [R]emove node, [D]iscord Username, [Q]uit"
            }
            Scene::Options => "none",
            Scene::DiscordUsernameInputBox => "⏎ Accept, [Esc] Cancel",
        };

        f.render_widget(
            Paragraph::new(text).block(
                Block::default()
                    .title(" Key commands ")
                    .borders(Borders::ALL),
            ),
            layer_zero[1],
        );

        Ok(())
    }
}
