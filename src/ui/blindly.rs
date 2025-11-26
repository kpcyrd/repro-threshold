use crate::app::App;
use crate::ui::SELECTED_STYLE;
use ratatui::{
    prelude::*,
    widgets::{
        Block, BorderType, HighlightSpacing, List, ListItem, Scrollbar, ScrollbarOrientation,
        ScrollbarState,
    },
};
use std::iter;

impl App {
    pub fn render_blindly_trust(&mut self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered()
            .title("repro-threshold")
            .title_alignment(Alignment::Center)
            .border_type(BorderType::Rounded);

        let items = iter::once(ListItem::from(Span::styled(
                "Use `repro-threshold plumbing [add-blindly-allow|remove-blindly-allow] <package>` to update",
                Style::new().add_modifier(Modifier::ITALIC)
            )))
            .chain(
                self.config
                    .rules
                    .blindly_allow
                    .iter()
                    .map(|s| ListItem::from(format!("Always blindly allow: {s}"))),
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
