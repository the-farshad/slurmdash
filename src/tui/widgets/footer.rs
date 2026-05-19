use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::{AppState, View};
use crate::tui::theme::Theme;

pub fn render(frame: &mut Frame<'_>, area: Rect, theme: &Theme, state: &AppState) {
    // Errors take precedence over the keybind hint and dismiss on the next
    // successful refresh.
    if let Some(err) = &state.last_error {
        let line = Line::from(vec![
            Span::styled(" ✘ ", Style::default().fg(theme.action_danger)),
            Span::styled(err.clone(), Style::default().fg(theme.action_danger)),
            Span::raw("  "),
            Span::styled(
                "(auto-clears on next refresh)",
                Style::default().fg(theme.muted),
            ),
        ]);
        frame.render_widget(Paragraph::new(line), area);
        return;
    }

    let keys = match state.view {
        View::Dashboard => {
            "1 dash  2 jobs   ↑↓ select   PgUp/PgDn page   Enter open   l logs   c cancel   h hold   Q requeue   Tab group   / filter   a me/all   s sort   ^K assist   R refresh   ? help   q quit"
        }
        View::Jobs => {
            "1 dash  2 jobs   ↑↓ select   PgUp/PgDn page   Enter open   l logs   c cancel   h hold   Q requeue   Tab group   / filter   a me/all   s sort   S reverse   ^K assist   R refresh   ? help   q quit"
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
