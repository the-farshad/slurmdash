//! Per-user job count panel. Shows a small horizontal bar chart of
//! jobs-per-user, sorted by count descending, with the user's average
//! wait time appended.

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

    // user -> (count, sum_wait_secs, wait_samples)
    let mut by_user: BTreeMap<String, (u32, u64, u32)> = BTreeMap::new();
    for j in jobs {
        let entry = by_user.entry(j.user.clone()).or_insert((0, 0, 0));
        entry.0 += 1;
        if let Some(w) = j.wait_seconds() {
            entry.1 += w;
            entry.2 += 1;
        }
    }

    let mut entries: Vec<(String, u32, u64, u32)> = by_user
        .into_iter()
        .map(|(k, (c, ws, wn))| (k, c, ws, wn))
        .collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let max = entries.iter().map(|e| e.1).max().unwrap_or(1);

    let rows = entries.len().min(inner.height as usize).max(1);
    if rows == 0 {
        return;
    }
    let constraints = vec![Constraint::Length(1); rows];
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    let name_w: u16 = 9;
    let count_w: u16 = 4;
    let wait_w: u16 = 7; // " ·  5m "
    let bar_w = inner
        .width
        .saturating_sub(name_w + count_w + wait_w + 1)
        .max(1) as usize;

    for (i, (user, count, sum_wait, wait_samples)) in entries.iter().take(rows).enumerate() {
        let pct = (*count as f64) / (max as f64);
        let filled = (pct * bar_w as f64).round() as usize;
        let fill: String = "▰".repeat(filled);
        let empty: String = "▱".repeat(bar_w.saturating_sub(filled));
        let wait_label = if *wait_samples > 0 {
            format!(" ·{:>5}", short_dur(sum_wait / *wait_samples as u64))
        } else {
            " ·    -".to_string()
        };
        let line = Line::from(vec![
            Span::styled(
                format!("{:<width$.width$}", user, width = name_w as usize),
                theme.footer_style(),
            ),
            Span::styled(fill, Style::default().fg(theme.accent)),
            Span::styled(empty, theme.footer_style()),
            Span::styled(format!(" {count:>3}"), Style::default().fg(theme.accent)),
            Span::styled(wait_label, theme.footer_style()),
        ]);
        frame.render_widget(Paragraph::new(line), chunks[i]);
    }
}

/// Format seconds as a short human-readable duration: "3s", "5m", "2h", "1d".
fn short_dur(s: u64) -> String {
    if s < 60 {
        format!("{s}s")
    } else if s < 3600 {
        format!("{}m", s / 60)
    } else if s < 86_400 {
        format!("{}h", s / 3600)
    } else {
        format!("{}d", s / 86_400)
    }
}
