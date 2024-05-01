// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::Component;
use crate::{
    action::{Action, TabActions},
    mode::Scene,
};
use color_eyre::Result;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Style, Stylize},
    widgets::Tabs,
};

pub struct Tab {
    scene_list: Vec<Scene>,
    current_tab_index: usize,
}

impl Default for Tab {
    fn default() -> Self {
        Self { scene_list: vec![Scene::Home, Scene::Options], current_tab_index: 0 }
    }
}

impl Tab {
    pub fn get_current_scene(&self) -> Scene {
        self.scene_list[self.current_tab_index]
    }
}

impl Component for Tab {
    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        let send_back = match action {
            Action::TabActions(TabActions::NextTab) => {
                info!(?self.current_tab_index, "Got Next tab");
                let mut new_index = self.current_tab_index + 1;
                if new_index >= self.scene_list.len() {
                    new_index = 0;
                }
                self.current_tab_index = new_index;
                let new_scene = self.scene_list[self.current_tab_index];
                info!(?new_scene, "Updated tab:");
                Some(Action::SwitchScene(new_scene))
            },

            Action::TabActions(TabActions::PreviousTab) => {
                info!(?self.current_tab_index, "Got PreviousTab");
                let new_index =
                    if self.current_tab_index == 0 { self.scene_list.len() - 1 } else { self.current_tab_index - 1 };
                self.current_tab_index = new_index;

                let new_scene = self.scene_list[self.current_tab_index];
                info!(?new_scene, "Updated tab:");
                Some(Action::SwitchScene(new_scene))
            },
            _ => None,
        };
        Ok(send_back)
    }

    fn draw(&mut self, f: &mut crate::tui::Frame<'_>, area: ratatui::prelude::Rect) -> Result<()> {
        let layer_zero = Layout::new(
            Direction::Vertical,
            [Constraint::Max(1), Constraint::Min(5), Constraint::Min(3), Constraint::Max(3)],
        )
        .split(area);
        let tab_items = self.scene_list.iter().map(|item| format!("{item:?}")).collect::<Vec<_>>();
        let tab = Tabs::new(tab_items)
            .style(Style::default().white())
            .highlight_style(Style::default().yellow())
            .select(self.current_tab_index)
            .divider("|")
            .padding(" ", " ");
        f.render_widget(tab, layer_zero[0]);

        Ok(())
    }
}
