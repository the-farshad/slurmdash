use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;

use crate::app::View;
use crate::tui::theme::Theme;

pub fn render(frame: &mut Frame<'_>, area: Rect, theme: &Theme, view: View) {
    let keys = match view {
        View::Jobs => "↑↓ select   Enter details   l logs   c cancel   h hold   u release   Q requeue   s sort   S reverse   R refresh   ? help   q quit",
        View::Details => "Esc back   q quit",
        View::Logs => "↑↓ jk scroll   PgUp/PgDn page   g top   G bottom   f follow   / search   n next   Esc back",
    };
    frame.render_widget(
        Paragraph::new(Line::raw(keys)).style(theme.footer_style()),
        area,
    );
}
