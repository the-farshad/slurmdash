//! Bucketed histogram of wait times across the current job set.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};

use crate::slurm::model::Job;
use crate::tui::theme::Theme;

type ColorFn = fn(&Theme) -> Color;
type Bucket = (&'static str, u64, u64, ColorFn);

const BUCKETS: &[Bucket] = &[
    ("< 1 min ", 0, 60, |t| t.usage_low),
    ("1–5 min ", 60, 300, |t| t.usage_low),
    ("5–30 min", 300, 1_800, |t| t.usage_med),
    ("30 min–2 h", 1_800, 7_200, |t| t.usage_med),
    ("2–12 h  ", 7_200, 43_200, |t| t.usage_high),
    ("> 12 h  ", 43_200, u64::MAX, |t| t.usage_critical),
];

pub fn render(frame: &mut Frame<'_>, area: Rect, jobs: &[Job], theme: &Theme) {
    let block = Block::default()
        .title(" Wait time distribution ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.border_style());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let counts: Vec<u32> = BUCKETS
        .iter()
        .map(|(_, lo, hi, _)| {
            jobs.iter()
                .filter_map(|j| j.wait_seconds())
                .filter(|w| w >= lo && w < hi)
                .count() as u32
        })
        .collect();
    let total: u32 = counts.iter().sum();
    if total == 0 {
        frame.render_widget(
            Paragraph::new(Line::styled(
                " (no jobs with wait time samples)",
                theme.footer_style(),
            )),
            inner,
        );
        return;
    }
    let max = *counts.iter().max().unwrap_or(&1);

    let rows = BUCKETS.len().min(inner.height as usize);
    let constraints = vec![Constraint::Length(1); rows];
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    let label_w: u16 = 12;
    let count_w: u16 = 6;
    let bar_w = inner.width.saturating_sub(label_w + count_w + 1).max(1) as usize;

    for (i, ((label, _, _, color_fn), count)) in
        BUCKETS.iter().zip(counts.iter()).enumerate().take(rows)
    {
        let pct = (*count as f64) / (max as f64);
        let color = color_fn(theme);
        let (fill, empty) = super::braille::bar_pair(pct, bar_w);
        let line = Line::from(vec![
            Span::styled(
                format!("{:<width$.width$}", label, width = label_w as usize),
                theme.footer_style(),
            ),
            Span::styled(fill, Style::default().fg(color)),
            Span::styled(empty, theme.border_style()),
            Span::styled(
                format!(" {count:>4}"),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
        ]);
        frame.render_widget(Paragraph::new(line), chunks[i]);
    }
}
