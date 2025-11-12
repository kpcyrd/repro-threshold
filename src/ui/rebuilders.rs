use crate::app::App;
use crate::rebuilder::{Rebuilder, Selectable};
use crate::ui::{
    ALT_ROW_BG_COLOR, COMPLETED_TEXT_FG_COLOR, NORMAL_ROW_BG, SELECTED_STYLE, TEXT_FG_COLOR,
};
use ratatui::{
    prelude::*,
    widgets::{
        Block, BorderType, HighlightSpacing, List, ListItem, Scrollbar, ScrollbarOrientation,
        ScrollbarState,
    },
};

impl App {
    pub fn render_rebuilders(&mut self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered()
            .title("repro-threshold")
            .title_alignment(Alignment::Center)
            .border_type(BorderType::Rounded)
            .bg(NORMAL_ROW_BG);

        let items: Vec<ListItem> = self
            .rebuilders
            .iter()
            .enumerate()
            .map(|(i, rebuilder)| {
                let color = alternate_colors(i);
                ListItem::from(rebuilder).bg(color)
            })
            .collect();

        let list = List::new(items)
            .block(block)
            .highlight_style(SELECTED_STYLE)
            .highlight_symbol(">")
            .highlight_spacing(HighlightSpacing::Always);

        StatefulWidget::render(&list, area, buf, &mut self.scroll);

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
                    .position(self.scroll.selected().unwrap_or_default()),
            );
    }
}

const fn alternate_colors(i: usize) -> Color {
    if i.is_multiple_of(2) {
        NORMAL_ROW_BG
    } else {
        ALT_ROW_BG_COLOR
    }
}

impl From<&Selectable<Rebuilder>> for ListItem<'_> {
    fn from(value: &Selectable<Rebuilder>) -> Self {
        let line = Line::from_iter([
            if value.active {
                Span::styled(" ✓", COMPLETED_TEXT_FG_COLOR)
            } else {
                Span::styled(" ☐", TEXT_FG_COLOR)
            },
            Span::raw(format!(" {:?}", value.item)),
        ]);

        ListItem::new(line)
    }
}
