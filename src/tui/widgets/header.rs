use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::{AppState, SortState};
use crate::tui::theme::Theme;

/// Braille spinner frames — one position per draw tick.
const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &AppState,
    theme: &Theme,
    cluster_label: &str,
    last_refresh: Option<chrono::DateTime<chrono::Utc>>,
    _refreshing: bool,
) {
    let refreshing = state.refresh.jobs_in_flight || state.refresh.sinfo_in_flight;
    let spinner = SPINNER[(state.frame as usize) % SPINNER.len()];
    let count = if state.text_filter.is_some() && state.jobs.len() != state.all_jobs.len() {
        format!("{}/{} jobs", state.jobs.len(), state.all_jobs.len())
    } else {
        format!("{} jobs", state.all_jobs.len().max(state.jobs.len()))
    };
    let status = if refreshing {
        format!("{spinner} refreshing")
    } else if let Some(t) = last_refresh {
        format!("updated {}", t.format("%H:%M:%S"))
    } else {
        "—".to_string()
    };
    let sort = format_sort(state.sort);
    let filter = format!("filter:{}", state.filter.label());
    let group = format!("group:{}", state.group_by.label());

    let mut spans = vec![
        // ▌ + `>_` is a stylized terminal-prompt mark — instantly tells the
        // viewer this is a CLI/TUI surface and not a web page screenshot.
        Span::styled("▌", Style::default().fg(theme.accent)),
        Span::styled(">_", theme.header_style()),
        Span::raw(" "),
        Span::styled("slurmdash", theme.header_style()),
        Span::raw("  "),
        Span::styled(cluster_label.to_string(), Style::default().fg(theme.accent)),
        Span::raw("  "),
        Span::styled(filter, Style::default().fg(theme.accent)),
        Span::raw("  "),
        Span::styled(count, Style::default().fg(theme.muted)),
        Span::raw("  "),
        Span::styled(status, Style::default().fg(theme.muted)),
        Span::raw("  "),
        Span::styled(sort, Style::default().fg(theme.muted)),
        Span::raw("  "),
        Span::styled(group, Style::default().fg(theme.muted)),
    ];

    // Live filter-input feedback, or the committed text filter.
    if let Some(buf) = &state.filter_input {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            format!("/{buf}_"),
            Style::default().fg(theme.accent),
        ));
    } else if let Some(f) = &state.text_filter {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            format!("search:{f}"),
            Style::default().fg(theme.accent),
        ));
    }

    let line = Line::from(spans);

    frame.render_widget(Paragraph::new(line), area);
}

fn format_sort(s: SortState) -> String {
    let arrow = if s.reverse { "↓" } else { "↑" };
    format!("sort:{}{}", s.key.label(), arrow)
}
