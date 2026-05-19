use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;

use crate::tui::theme::Theme;

pub fn render(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let keys = "↑/↓ select   R refresh   /? help   q quit";
    frame.render_widget(
        Paragraph::new(Line::raw(keys)).style(theme.footer_style()),
        area,
    );
}
