//! Per-user table with state breakdown — fuller version of by_user.rs,
//! intended for the Statistics page.

use std::collections::BTreeMap;

use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Cell, Row, Table};

use crate::slurm::model::Job;
use crate::slurm::state::JobState;
use crate::tui::theme::Theme;

#[derive(Default)]
struct UserSummary {
    total: u32,
    running: u32,
    pending: u32,
    failed: u32,
    nodes: u32,
    wait_sum: u64,
    wait_n: u32,
}

pub fn render(frame: &mut Frame<'_>, area: Rect, jobs: &[Job], theme: &Theme) {
    let block = Block::default()
        .title(" Top users ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.border_style());
    frame.render_widget(block.clone(), area);
    let inner = block.inner(area);

    let mut by_user: BTreeMap<String, UserSummary> = BTreeMap::new();
    for j in jobs {
        let e = by_user.entry(j.user.clone()).or_default();
        e.total += 1;
        e.nodes = e.nodes.saturating_add(j.nodes);
        match j.state {
            JobState::Running => e.running += 1,
            JobState::Pending => e.pending += 1,
            JobState::Failed
            | JobState::Timeout
            | JobState::NodeFail
            | JobState::BootFail
            | JobState::Deadline
            | JobState::OutOfMemory => e.failed += 1,
            _ => {}
        }
        if let Some(w) = j.wait_seconds() {
            e.wait_sum = e.wait_sum.saturating_add(w);
            e.wait_n += 1;
        }
    }

    let mut rows_data: Vec<(String, UserSummary)> = by_user.into_iter().collect();
    rows_data.sort_by(|a, b| b.1.total.cmp(&a.1.total).then_with(|| a.0.cmp(&b.0)));
    let max_total = rows_data.iter().map(|r| r.1.total).max().unwrap_or(1) as f64;
    let bar_w_chars: usize = 16;

    let header_cells = ["USER", "JOBS", "BAR", "R", "PD", "F", "NODES", "AVG WAIT"];
    let header = Row::new(
        header_cells
            .into_iter()
            .map(|h| Cell::from(Span::styled(h, Style::default().fg(theme.accent).bold()))),
    );

    let body: Vec<Row> = rows_data
        .iter()
        .take(inner.height.saturating_sub(1) as usize)
        .map(|(user, s)| {
            let pct = (s.total as f64) / max_total;
            let filled = (pct * bar_w_chars as f64).round() as usize;
            let fill = "▰".repeat(filled);
            let empty = "▱".repeat(bar_w_chars - filled);
            let wait = if s.wait_n > 0 {
                short_dur(s.wait_sum / s.wait_n as u64)
            } else {
                "—".to_string()
            };
            Row::new(vec![
                Cell::from(Span::styled(
                    user.clone(),
                    Style::default().fg(theme.accent),
                )),
                Cell::from(Span::styled(
                    format!("{}", s.total),
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                )),
                Cell::from(Line::from(vec![
                    Span::styled(fill, Style::default().fg(theme.accent)),
                    Span::styled(empty, Style::default().fg(theme.border)),
                ])),
                Cell::from(Span::styled(
                    format!("{}", s.running),
                    Style::default().fg(theme.running),
                )),
                Cell::from(Span::styled(
                    format!("{}", s.pending),
                    Style::default().fg(theme.pending),
                )),
                Cell::from(Span::styled(
                    format!("{}", s.failed),
                    Style::default().fg(theme.failed),
                )),
                Cell::from(format!("{}", s.nodes)),
                Cell::from(wait),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(14),
        Constraint::Length(5),
        Constraint::Length(bar_w_chars as u16 + 1),
        Constraint::Length(4),
        Constraint::Length(4),
        Constraint::Length(4),
        Constraint::Length(6),
        Constraint::Fill(1),
    ];

    let table = Table::new(body, widths).header(header.height(1));
    frame.render_widget(table, inner);
}

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
