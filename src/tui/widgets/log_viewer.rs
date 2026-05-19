use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::{LogView, SortState};
use crate::tui::theme::Theme;

/// Top status line for the log viewer (path, follow/pause state, search).
pub fn render_header(
    frame: &mut Frame<'_>,
    area: Rect,
    log: &LogView,
    theme: &Theme,
    typing_search: Option<&str>,
    _sort: SortState,
) {
    let follow = if log.follow { "FOLLOW" } else { "PAUSED" };
    let follow_color = if log.follow {
        theme.action_normal
    } else {
        theme.action_warning
    };
    let mut spans = vec![
        Span::styled(format!(" log "), theme.header_style()),
        Span::raw(format!("{}  ", log.kind.label())),
        Span::styled(log.path.clone(), Style::default().fg(theme.muted)),
        Span::raw("    "),
        Span::styled(follow, Style::default().fg(follow_color).add_modifier(Modifier::BOLD)),
        Span::raw("    "),
        Span::styled(format!("{} lines", log.lines.len()), Style::default().fg(theme.muted)),
    ];
    if let Some(q) = typing_search {
        spans.push(Span::raw("    /"));
        spans.push(Span::styled(q.to_string(), Style::default().fg(theme.accent)));
        spans.push(Span::raw("_"));
    } else if let Some(q) = &log.search {
        spans.push(Span::raw("    search="));
        spans.push(Span::styled(q.clone(), Style::default().fg(theme.accent)));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// Body: paged window into log.lines.
pub fn render_body(frame: &mut Frame<'_>, area: Rect, log: &LogView, theme: &Theme) {
    let block = Block::default()
        .borders(Borders::TOP | Borders::BOTTOM)
        .border_style(theme.border_style());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let visible_rows = inner.height as usize;
    if visible_rows == 0 {
        return;
    }

    let total = log.lines.len();
    let start = if log.follow {
        total.saturating_sub(visible_rows)
    } else {
        log.scroll.min(total.saturating_sub(1))
    };
    let end = (start + visible_rows).min(total);

    let lines: Vec<Line> = log
        .lines
        .iter()
        .skip(start)
        .take(end - start)
        .map(|raw| match &log.search {
            Some(q) if !q.is_empty() && raw.contains(q.as_str()) => highlight(raw, q, theme),
            _ => Line::raw(raw.clone()),
        })
        .collect();

    frame.render_widget(Paragraph::new(lines), inner);
}

fn highlight<'a>(line: &'a str, query: &str, theme: &Theme) -> Line<'a> {
    let mut spans = Vec::new();
    let mut rest = line;
    while let Some(idx) = rest.find(query) {
        if idx > 0 {
            spans.push(Span::raw(rest[..idx].to_string()));
        }
        spans.push(Span::styled(
            query.to_string(),
            Style::default().fg(theme.bg).bg(theme.accent),
        ));
        rest = &rest[idx + query.len()..];
    }
    if !rest.is_empty() {
        spans.push(Span::raw(rest.to_string()));
    }
    Line::from(spans)
}
