use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;

use crate::app::View;
use crate::tui::theme::Theme;

pub fn render(frame: &mut Frame<'_>, area: Rect, theme: &Theme, view: View) {
    let keys = match view {
        View::Dashboard => {
            "1 dash  2 jobs   ↑↓ select   Enter details   l logs   c cancel   h hold   Q requeue   / filter   a me/all   s sort   ^K assist   R refresh   ? help   q quit"
        }
        View::Jobs => {
            "1 dash  2 jobs   ↑↓ select   Enter details   l logs   c cancel   h hold   Q requeue   / filter   a me/all   s sort   S reverse   ^K assist   R refresh   ? help   q quit"
        }
        View::Details => "Esc back   q quit",
        View::Logs => {
            "↑↓ jk scroll   PgUp/PgDn page   g top   G bottom   f follow   / search   n next   Esc back"
        }
    };
    frame.render_widget(
        Paragraph::new(Line::raw(keys)).style(theme.footer_style()),
        area,
    );
}
