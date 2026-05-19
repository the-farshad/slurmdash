//! Queue panel: count of jobs by state, with colored bars.

use std::collections::BTreeMap;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::slurm::model::Job;
use crate::slurm::state::JobState;
use crate::tui::theme::Theme;

pub fn render(frame: &mut Frame<'_>, area: Rect, jobs: &[Job], theme: &Theme) {
    let block = Block::default()
        .title(" Queue ")
        .borders(Borders::ALL)
        .border_style(theme.border_style());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let counts = group_by_state(jobs);
    let max = counts.iter().map(|(_, c)| *c).max().unwrap_or(0);
    if counts.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::styled("(no jobs)", theme.footer_style())),
            inner,
        );
        return;
    }

    let rows = counts.len().min(inner.height as usize).max(1);
    let constraints = vec![Constraint::Length(1); rows];
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    for (i, (state_label, count)) in counts.iter().take(rows).enumerate() {
        bar_line(frame, chunks[i], state_label, *count, max, theme);
    }
}

fn bar_line(
    frame: &mut Frame<'_>,
    area: Rect,
    state_label: &str,
    count: u64,
    max: u64,
    theme: &Theme,
) {
    let state = JobState::parse(state_label);
    let color = theme.job_state_style(&state).fg.unwrap_or(theme.fg);
    let pct = if max == 0 {
        0.0
    } else {
        count as f64 / max as f64
    };

    let reserved = 10 + 1 + 6 + 1;
    let bar_w = area.width.saturating_sub(reserved) as usize;
    let filled = (pct * bar_w as f64).round() as usize;
    let fill: String = "▰".repeat(filled);
    let empty: String = "▱".repeat(bar_w.saturating_sub(filled));

    let line = Line::from(vec![
        Span::styled(format!("{state_label:<10}"), Style::default().fg(color)),
        Span::styled(fill, Style::default().fg(color)),
        Span::styled(empty, theme.footer_style()),
        Span::raw(" "),
        Span::styled(format!("{count:>5}"), Style::default().fg(color)),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

/// Group jobs by JobState into an ordered map, with running first then
/// pending, then everything else alphabetically.
fn group_by_state(jobs: &[Job]) -> Vec<(String, u64)> {
    let mut by: BTreeMap<String, u64> = BTreeMap::new();
    for j in jobs {
        *by.entry(j.state.short().to_string()).or_insert(0) += 1;
    }
    let priority = ["R", "PD", "CG", "S", "H"];
    let mut out: Vec<(String, u64)> = Vec::new();
    for p in priority {
        if let Some(&c) = by.get(p) {
            out.push((p.to_string(), c));
            by.remove(p);
        }
    }
    let mut rest: Vec<(String, u64)> = by.into_iter().collect();
    rest.sort_by(|a, b| a.0.cmp(&b.0));
    out.extend(rest);
    out
}
