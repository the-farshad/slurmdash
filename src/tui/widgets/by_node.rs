//! Per-node bar chart panel. Counts the number of running jobs on each
//! individual host by expanding `Job.reason_or_nodelist` through Slurm's
//! hostlist syntax (see [`crate::slurm::hostlist::expand`]).

use std::collections::BTreeMap;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};

use crate::slurm::hostlist;
use crate::slurm::model::Job;
use crate::slurm::state::JobState;
use crate::tui::theme::Theme;

pub fn render(frame: &mut Frame<'_>, area: Rect, jobs: &[Job], theme: &Theme) {
    let block = Block::default()
        .title(" By node ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.border_style());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut by_node: BTreeMap<String, u32> = BTreeMap::new();
    for j in jobs {
        if j.state != JobState::Running {
            continue;
        }
        for host in hostlist::expand(&j.reason_or_nodelist) {
            *by_node.entry(host).or_insert(0) += 1;
        }
    }

    if by_node.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::styled(
                " (no running jobs on resolvable nodes)",
                theme.footer_style(),
            )),
            inner,
        );
        return;
    }

    let mut entries: Vec<(String, u32)> = by_node.into_iter().collect();
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

    let name_w: u16 = 12;
    let count_w: u16 = 5;
    let bar_w = inner.width.saturating_sub(name_w + count_w + 1).max(1) as usize;

    for (i, (node, count)) in entries.iter().take(rows).enumerate() {
        let pct = (*count as f64) / (max as f64);
        let (fill, empty) = super::braille::bar_pair(pct, bar_w);
        let line = Line::from(vec![
            Span::styled(
                format!("{:<width$.width$}", node, width = name_w as usize),
                theme.footer_style(),
            ),
            Span::styled(fill, Style::default().fg(theme.accent)),
            Span::styled(empty, theme.footer_style()),
            Span::styled(format!(" {count:>4}"), Style::default().fg(theme.accent)),
        ]);
        frame.render_widget(Paragraph::new(line), chunks[i]);
    }
}
