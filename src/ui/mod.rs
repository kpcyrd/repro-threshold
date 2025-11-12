mod home;
mod rebuilders;

use crate::app::App;
use ratatui::{
    layout::Flex,
    prelude::*,
    style::palette::tailwind::{GREEN, SLATE},
    widgets::{Block, Clear},
};

const NORMAL_ROW_BG: Color = SLATE.c950;
const ALT_ROW_BG_COLOR: Color = SLATE.c900;
const SELECTED_STYLE: Style = Style::new().bg(SLATE.c800).add_modifier(Modifier::BOLD);
const TEXT_FG_COLOR: Color = SLATE.c200;
const COMPLETED_TEXT_FG_COLOR: Color = GREEN.c500;

impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        match self.view {
            Some(crate::app::View::Home) => self.render_home(area, buf),
            Some(crate::app::View::Rebuilders) => self.render_rebuilders(area, buf),
            None => {}
        }

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
