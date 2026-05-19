//! Compact list of running jobs whose time-limit is almost up. Useful as a
//! "what will free soon" signal next to the resource bars.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};

use crate::slurm::model::Job;
use crate::slurm::state::JobState;
use crate::tui::format::hms;
use crate::tui::theme::Theme;

/// Maximum number of rows to show.
const MAX_ROWS: usize = 8;

pub fn render(frame: &mut Frame<'_>, area: Rect, jobs: &[Job], theme: &Theme) {
    let block = Block::default()
        .title(" Ending soon ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.border_style());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut candidates: Vec<(&Job, u64)> = jobs
        .iter()
        .filter_map(|j| {
            if j.state != JobState::Running {
                return None;
            }
            let limit = j.time_limit_seconds?;
            let elapsed = j.elapsed_seconds?;
            let remaining = limit.saturating_sub(elapsed);
            Some((j, remaining))
        })
        .collect();
    candidates.sort_by_key(|(_, r)| *r);
    candidates.truncate(MAX_ROWS.min(inner.height as usize));

    if candidates.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::styled(" (no running jobs)", theme.footer_style())),
            inner,
        );
        return;
    }

    let lines: Vec<Line> = candidates
        .into_iter()
        .map(|(j, remaining)| {
            let color = if remaining < 5 * 60 {
                theme.usage_critical
            } else if remaining < 30 * 60 {
                theme.usage_high
            } else {
                theme.usage_med
            };
            Line::from(vec![
                Span::styled(
                    format!(" {:<8}", j.job_id),
                    Style::default().fg(theme.accent),
                ),
                Span::styled(format!("{:<14.14} ", j.name), Style::default().fg(theme.fg)),
                Span::styled(format!("-{}", hms(remaining)), Style::default().fg(color)),
            ])
        })
        .collect();

    frame.render_widget(Paragraph::new(lines), inner);
}
