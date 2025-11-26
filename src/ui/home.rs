use crate::app::App;
use crate::ui::{COLOR_NEGATIVE, COLOR_POSITIVE, COLOR_WARNING, SELECTED_STYLE};
use ratatui::{
    prelude::*,
    widgets::{Block, BorderType, HighlightSpacing, List, ListItem},
};

impl App {
    pub fn render_home(&mut self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered()
            .title("repro-threshold")
            .title_alignment(Alignment::Center)
            .border_type(BorderType::Rounded);

        let required_threshold = self.config.rules.required_threshold;
        let trusted_rebuilders = self.config.trusted_rebuilders.len();

        let items = vec![
            ListItem::new(Line::from_iter([
                Span::raw(" Required reproduction threshold: "),
                Span::styled(
                    required_threshold.to_string(),
                    match required_threshold {
                        0 => COLOR_NEGATIVE,
                        1 => COLOR_WARNING,
                        _ => COLOR_POSITIVE,
                    },
                ),
                Span::raw("/"),
                Span::raw(format!("{trusted_rebuilders}")),
            ])),
            ListItem::new(format!(
                " Configure trusted rebuilders ({trusted_rebuilders} selected)"
            )),
            ListItem::new(format!(
                " Add/remove packages from 'blindly trust' allow-list ({} entries)",
                self.config.rules.blindly_allow.len()
            )),
            ListItem::new(" Quit"),
        ];

        let list = List::new(items)
            .block(block)
            .highlight_style(SELECTED_STYLE)
            .highlight_symbol(">")
            .highlight_spacing(HighlightSpacing::Always);

        StatefulWidget::render(&list, area, buf, self.scroll());
    }
}
