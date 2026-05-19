use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::AppState;
use crate::tui::theme::Theme;

pub fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &AppState,
    theme: &Theme,
    cluster_label: &str,
    last_refresh: Option<chrono::DateTime<chrono::Utc>>,
    refreshing: bool,
) {
    let count = format!("{} jobs", state.jobs.len());
    let status = if refreshing {
        "refreshing…".to_string()
    } else if let Some(t) = last_refresh {
        format!("updated {}", t.format("%H:%M:%S"))
    } else {
        "—".to_string()
    };

    let line = Line::from(vec![
        Span::styled("slurmdash", theme.header_style()),
        Span::raw("  "),
        Span::styled(cluster_label.to_string(), Style::default().fg(theme.accent)),
        Span::raw("  "),
        Span::styled(count, Style::default().fg(theme.muted)),
        Span::raw("  "),
        Span::styled(status, Style::default().fg(theme.muted)),
    ]);

    frame.render_widget(Paragraph::new(line), area);
}
