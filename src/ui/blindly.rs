use crate::app::App;
use crate::ui::{self, SELECTED_STYLE};
use ratatui::{
    prelude::*,
    widgets::{HighlightSpacing, List, ListItem, Scrollbar, ScrollbarOrientation, ScrollbarState},
};
use std::iter;

impl App {
    pub fn render_blindly_trust(&mut self, area: Rect, buf: &mut Buffer) {
        let block = ui::container();

        let items = iter::once(ListItem::from(Span::styled(
                "Use `repro-threshold plumbing [add-blindly-trust|remove-blindly-trust] <package>` to update",
                Style::new().italic()
            )))
            .chain(
                self.config
                    .rules
                    .blindly_trust
                    .iter()
                    .map(|s| ListItem::from(format!("Always blindly trust: {s}"))),
            )
            .collect::<Vec<_>>();

        let list = List::new(items)
            .block(block)
            .highlight_style(SELECTED_STYLE)
            .highlight_symbol("> ")
            .highlight_spacing(HighlightSpacing::Always);

        StatefulWidget::render(&list, area, buf, self.scroll());

        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .track_symbol(None)
            .render(
                area.inner(Margin {
                    horizontal: 0,
                    vertical: 1,
                }),
                buf,
                &mut ScrollbarState::new(list.len())
                    .position(self.scroll().selected().unwrap_or_default()),
            );
    }
}
