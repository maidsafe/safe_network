// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::style::{EUCALYPTUS, GHOST_WHITE, LIGHT_PERIWINKLE};
use ratatui::{prelude::*, widgets::*};

pub enum NodesToStart {
    Configured,
    NotConfigured,
}

#[derive(Default)]
pub struct Footer {}

impl StatefulWidget for Footer {
    type State = NodesToStart;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let (text_style, command_style) = if matches!(state, NodesToStart::Configured) {
            (
                Style::default().fg(EUCALYPTUS),
                Style::default().fg(GHOST_WHITE),
            )
        } else {
            (
                Style::default().fg(LIGHT_PERIWINKLE),
                Style::default().fg(LIGHT_PERIWINKLE),
            )
        };

        let command1 = vec![
            Span::styled("[Ctrl+G] ", Style::default().fg(GHOST_WHITE)),
            Span::styled("Manage Nodes", Style::default().fg(EUCALYPTUS)),
        ];
        let command2 = vec![
            Span::styled("[Ctrl+S] ", command_style),
            Span::styled("Start Nodes", text_style),
        ];
        let command3 = vec![
            Span::styled("[Ctrl+X] ", command_style),
            Span::styled("Stop Nodes", text_style),
        ];

        let cell1 = Cell::from(Line::from(command1));
        let cell2 = Cell::from(Line::from(command2));
        let cell3 = Cell::from(Line::from(command3));
        let row = Row::new(vec![cell1, cell2, cell3]);

        let table = Table::new(vec![row], vec![Constraint::Max(1)])
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(EUCALYPTUS))
                    .padding(Padding::horizontal(1)),
            )
            .widths(vec![
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
            ]);

        StatefulWidget::render(table, area, buf, &mut TableState::default());
    }
}
