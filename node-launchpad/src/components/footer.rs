// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use std::borrow::Cow;

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

        let text: Cow<_> = match self.current_scene {
            Scene::Home => {
                "[Ctrl+g] Start nodes, [Ctrl+x] Stop node, [O] Set Resources, [D]iscord Username, [Q]uit".into()
            }
            Scene::Options => "none".into(),
            Scene::DiscordUsernameInputBox => "⏎ Accept, [Esc] Cancel".into(),
            Scene::ResourceAllocationInputBox => format!("⏎ Accept, [Esc] Cancel.").into(),
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
