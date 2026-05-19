//! Headline KPI strip. One line of bold counters across the top of the
//! Statistics view: jobs, state breakdown, nodes/GPUs in use, mean wait.
//! Each chip is styled to match its semantic color (running = green,
//! pending = yellow, failed = red, etc.) so the user can read the
//! cluster's pulse at a glance.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};

use crate::slurm::model::{ClusterResources, Job};
use crate::slurm::state::JobState;
use crate::tui::theme::Theme;

pub fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    jobs: &[Job],
    resources: &ClusterResources,
    theme: &Theme,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.border_style())
        .title(" Cluster pulse ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let stats = compute(jobs);
    let total_gpus_alloc: u32 = resources.gpus.allocated;
    let total_nodes_alloc: u32 = resources.nodes.allocated;
    let total_nodes: u32 = resources.nodes.total;

    let chips: Vec<Chip> = vec![
        Chip::new("JOBS", stats.total.to_string(), theme.accent),
        Chip::new("▶ RUN", stats.running.to_string(), theme.running),
        Chip::new("◷ PEND", stats.pending.to_string(), theme.pending),
        Chip::new("‖ HOLD", stats.held.to_string(), theme.held),
        Chip::new("✘ FAIL", stats.failed.to_string(), theme.failed),
        Chip::new(
            "NODES",
            format!("{}/{}", total_nodes_alloc, total_nodes),
            theme.fg,
        ),
        Chip::new("GPUs", format!("{}", total_gpus_alloc), theme.action_normal),
        Chip::new("Σ GPU·JOB", stats.gpu_jobs.to_string(), theme.action_normal),
        Chip::new("MEAN WAIT", fmt_dur(stats.wait_mean), theme.action_warning),
    ];

    let n = chips.len() as u32;
    let constraints: Vec<Constraint> = (0..n).map(|_| Constraint::Ratio(1, n)).collect();
    let slots = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(inner);

    for (chip, slot) in chips.into_iter().zip(slots.iter()) {
        frame.render_widget(
            Paragraph::new(chip.render(theme)).alignment(ratatui::layout::Alignment::Center),
            *slot,
        );
    }
}

struct Chip {
    label: &'static str,
    value: String,
    color: Color,
}

impl Chip {
    fn new(label: &'static str, value: String, color: Color) -> Self {
        Self {
            label,
            value,
            color,
        }
    }

    fn render(&self, theme: &Theme) -> Vec<Line<'_>> {
        vec![
            Line::from(Span::styled(self.label, Style::default().fg(theme.muted))),
            Line::from(Span::styled(
                self.value.clone(),
                Style::default().fg(self.color).add_modifier(Modifier::BOLD),
            )),
        ]
    }
}

struct Stats {
    total: u32,
    running: u32,
    pending: u32,
    held: u32,
    failed: u32,
    gpu_jobs: u32,
    wait_mean: u64,
}

fn compute(jobs: &[Job]) -> Stats {
    let mut s = Stats {
        total: jobs.len() as u32,
        running: 0,
        pending: 0,
        held: 0,
        failed: 0,
        gpu_jobs: 0,
        wait_mean: 0,
    };
    let mut wait_sum: u64 = 0;
    let mut wait_n: u32 = 0;
    for j in jobs {
        match j.state {
            JobState::Running => s.running += 1,
            JobState::Pending => s.pending += 1,
            JobState::Held => s.held += 1,
            JobState::Failed
            | JobState::Timeout
            | JobState::NodeFail
            | JobState::BootFail
            | JobState::Deadline
            | JobState::OutOfMemory => s.failed += 1,
            _ => {}
        }
        if j.uses_gpu() {
            s.gpu_jobs += 1;
        }
        if let Some(w) = j.wait_seconds() {
            wait_sum = wait_sum.saturating_add(w);
            wait_n += 1;
        }
    }
    if wait_n > 0 {
        s.wait_mean = wait_sum / wait_n as u64;
    }
    s
}

fn fmt_dur(s: u64) -> String {
    if s == 0 {
        "—".into()
    } else if s < 60 {
        format!("{s}s")
    } else if s < 3600 {
        format!("{}m", s / 60)
    } else if s < 86_400 {
        format!("{}h", s / 3600)
    } else {
        format!("{}d", s / 86_400)
    }
}
