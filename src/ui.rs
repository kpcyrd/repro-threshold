use crate::app::App;
use crate::rebuilder::{Rebuilder, Selectable};
use ratatui::{
    layout::Flex,
    prelude::*,
    style::palette::tailwind::{GREEN, SLATE},
    widgets::{
        Block, BorderType, Clear, HighlightSpacing, List, ListItem, Scrollbar,
        ScrollbarOrientation, ScrollbarState,
    },
};

const NORMAL_ROW_BG: Color = SLATE.c950;
const ALT_ROW_BG_COLOR: Color = SLATE.c900;
const SELECTED_STYLE: Style = Style::new().bg(SLATE.c800).add_modifier(Modifier::BOLD);
const TEXT_FG_COLOR: Color = SLATE.c200;
const COMPLETED_TEXT_FG_COLOR: Color = GREEN.c500;

impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered()
            .title("repro-threshold")
            .title_alignment(Alignment::Center)
            .border_type(BorderType::Rounded)
            .bg(NORMAL_ROW_BG);

        /*
        let text = vec![
            Line::from(" ✓ DONE: This is a line ".green()),
            Line::from(" ☐ TODO: This is a line   "),
            Line::from("This is a line".on_dark_gray()),
            Line::from("This is a line "),
            Line::from("This is a line   ".red()),
            Line::from("This is a line".on_dark_gray()),
            Line::from("This is a line "),
            Line::from("This is a line   ".red()),
            Line::from("This is a line".on_dark_gray()),
            Line::from("This is a line "),
            Line::from("This is a line "),
            Line::from("This is a line "),
            Line::from("This is a line "),
            Line::from("This is a line "),
            Line::from("This is a line   ".red()),
            Line::from("This is a line".on_dark_gray()),
            Line::from("This is a longer line".crossed_out()),
            Line::from("This is a line".reset()),
            Line::from(vec![
                Span::raw("Masked text: "),
                Span::styled(Masked::new("password", '*'), Style::new().fg(Color::Red)),
            ]),
            Line::from("This is a line "),
            Line::from("This is a line   ".red()),
            Line::from("This is a line".on_dark_gray()),
            Line::from("This is a longer line".crossed_out()),
            Line::from("This is a line".reset()),
            Line::from(vec![
                Span::raw("Masked text: "),
                Span::styled(Masked::new("password", '*'), Style::new().fg(Color::Red)),
            ]),
        ];
        */

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

        //

        if self.confirm {
            let popup = Block::bordered().title("Are you sure?");
            let popup_area = centered_area(area, 60, 40);
            // clears out any background in the area before rendering the popup
            Clear.render(popup_area, buf);
            popup.render(popup_area, buf);
        }
    }
}

fn centered_area(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let vertical = Layout::vertical([Constraint::Percentage(percent_y)]).flex(Flex::Center);
    let horizontal = Layout::horizontal([Constraint::Percentage(percent_x)]).flex(Flex::Center);
    let [area] = area.layout(&vertical);
    let [area] = area.layout(&horizontal);
    area
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
