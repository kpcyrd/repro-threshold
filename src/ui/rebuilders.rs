use crate::app::App;
use crate::rebuilder::{Rebuilder, Selectable};
use crate::ui::{self, COLOR_POSITIVE, SELECTED_STYLE};
use ratatui::{
    prelude::*,
    widgets::{HighlightSpacing, List, ListItem, Scrollbar, ScrollbarOrientation, ScrollbarState},
};

impl App {
    pub fn render_rebuilders(&mut self, area: Rect, buf: &mut Buffer) {
        let block = ui::container();

        let items = if self.rebuilders.is_empty() {
            vec![ListItem::new(Span::styled(
                "No rebuilders configured, press ctrl-R to load community set, or run `repro-threshold plumbing add-rebuilder <url>` to add one",
                Style::new().italic(),
            ))]
        } else {
            self.rebuilders
                .iter()
                .map(ListItem::from)
                .collect::<Vec<_>>()
        };

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

impl From<&Selectable<Rebuilder>> for ListItem<'_> {
    fn from(value: &Selectable<Rebuilder>) -> Self {
        let mut line = Line::from_iter([
            if value.active {
                Span::styled("✓", COLOR_POSITIVE)
            } else {
                Span::raw("☐")
            },
            Span::raw(format!(
                " {} - {}",
                value.item.name.escape_default(),
                value.item.url
            )),
        ]);

        if !value.item.distributions.is_empty() {
            line.push_span(Span::raw(" ["));
            for (i, dist) in value.item.distributions.iter().enumerate() {
                if i > 0 {
                    line.push_span(Span::raw(", "));
                }
                line.push_span(Span::raw(dist.escape_default().to_string()));
            }
            line.push_span(Span::raw("]"));
        }

        if let Ok(key) = value.item.signing_key() {
            line.push_span(Span::raw(format!(" - {:?}", key.key_id())));
        }

        ListItem::new(line)
    }
}
