use crate::app::App;
use crate::ui::{NORMAL_ROW_BG, SELECTED_STYLE};
use ratatui::{
    prelude::*,
    widgets::{Block, BorderType, HighlightSpacing, List, ListItem},
};

impl App {
    pub fn render_home(&mut self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered()
            .title("repro-threshold")
            .title_alignment(Alignment::Center)
            .border_type(BorderType::Rounded)
            .bg(NORMAL_ROW_BG);

        let items = vec![
            ListItem::new(" Required reproduction threshold: 123/456"),
            ListItem::new(" Configure trusted rebuilders (1234 selected)"),
            ListItem::new(" Add/remove packages from 'blindly trust' allow-list (12345 entries)"),
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
