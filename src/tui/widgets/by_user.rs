//! Per-user job count panel. Shows a small horizontal bar chart of
//! jobs-per-user, sorted by count descending.

use std::collections::BTreeMap;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::slurm::model::Job;
use crate::tui::theme::Theme;

pub fn render(frame: &mut Frame<'_>, area: Rect, jobs: &[Job], theme: &Theme) {
    let block = Block::default()
        .title(" By user ")
        .borders(Borders::ALL)
        .border_style(theme.border_style());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if jobs.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::styled(" (no jobs)", theme.footer_style())),
            inner,
        );
        return;
    }

    let mut by_user: BTreeMap<String, u32> = BTreeMap::new();
    for j in jobs {
        *by_user.entry(j.user.clone()).or_insert(0) += 1;
    }

    let mut entries: Vec<(String, u32)> = by_user.into_iter().collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let max = entries.iter().map(|(_, c)| *c).max().unwrap_or(1);

    let rows = entries.len().min(inner.height as usize).max(1);
    if rows == 0 {
        return;
    }
    let constraints = vec![Constraint::Length(1); rows];
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    let name_w: u16 = 10;
    let count_w: u16 = 5;
    let bar_w = inner.width.saturating_sub(name_w + count_w + 1).max(1) as usize;

    for (i, (user, count)) in entries.iter().take(rows).enumerate() {
        let pct = (*count as f64) / (max as f64);
        let filled = (pct * bar_w as f64).round() as usize;
        let fill: String = "█".repeat(filled);
        let empty: String = "░".repeat(bar_w.saturating_sub(filled));
        let line = Line::from(vec![
            Span::styled(
                format!("{:<width$.width$}", user, width = name_w as usize),
                theme.footer_style(),
            ),
            Span::styled(fill, Style::default().fg(theme.accent)),
            Span::styled(empty, theme.footer_style()),
            Span::styled(format!(" {count:>4}"), Style::default().fg(theme.accent)),
        ]);
        frame.render_widget(Paragraph::new(line), chunks[i]);
    }
}
