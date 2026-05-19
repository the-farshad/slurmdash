//! Cluster-wide resource panel: CPU / GPU / memory / nodes bars.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::slurm::model::ClusterResources;
use crate::tui::theme::Theme;

pub fn render(frame: &mut Frame<'_>, area: Rect, r: &ClusterResources, theme: &Theme) {
    let block = Block::default()
        .title(" Resources ")
        .borders(Borders::ALL)
        .border_style(theme.border_style());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // One line per resource type. Skip GPU row if the cluster has none.
    let has_gpu = r.gpus.total > 0;
    let rows = if has_gpu { 4 } else { 3 };
    let constraints = vec![Constraint::Length(1); rows];
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    bar_line(
        frame,
        chunks[0],
        "CPU",
        r.cpus.allocated as u64,
        r.cpus.total as u64,
        format!("{}/{}", r.cpus.allocated, r.cpus.total),
        theme,
    );
    let mut row = 1;
    if has_gpu {
        bar_line(
            frame,
            chunks[row],
            "GPU",
            r.gpus.allocated as u64,
            r.gpus.total as u64,
            format!("{}/{}", r.gpus.allocated, r.gpus.total),
            theme,
        );
        row += 1;
    }
    bar_line(
        frame,
        chunks[row],
        "MEM",
        r.memory_mb.allocated,
        r.memory_mb.total,
        format!(
            "{} / {}",
            humanize_mb(r.memory_mb.allocated),
            humanize_mb(r.memory_mb.total)
        ),
        theme,
    );
    row += 1;

    let node_line = Line::from(vec![
        Span::styled(format!("{:<5}", "NODE"), theme.footer_style()),
        Span::styled(format!("alloc {}  ", r.nodes.allocated), Style::default().fg(theme.usage_high)),
        Span::styled(format!("idle {}  ", r.nodes.idle), Style::default().fg(theme.usage_low)),
        Span::styled(format!("other {}  ", r.nodes.other), Style::default().fg(theme.usage_med)),
        Span::styled(format!("total {}", r.nodes.total), Style::default().fg(theme.muted)),
    ]);
    frame.render_widget(Paragraph::new(node_line), chunks[row]);
}

fn bar_line(
    frame: &mut Frame<'_>,
    area: Rect,
    label: &str,
    done: u64,
    total: u64,
    suffix: String,
    theme: &Theme,
) {
    let pct = if total == 0 { 0.0 } else { (done as f64 / total as f64).clamp(0.0, 1.0) };
    let color = gradient(pct, theme);

    // Reserve space for label (5) + " " + "[ ]" + " " + pct (5) + " " + suffix
    let reserved = 5 + 1 + 2 + 1 + 5 + 1 + suffix.len() as u16;
    let bar_w = area.width.saturating_sub(reserved) as usize;
    let filled = (pct * bar_w as f64).round() as usize;
    let fill: String = "█".repeat(filled);
    let empty: String = "░".repeat(bar_w.saturating_sub(filled));

    let line = Line::from(vec![
        Span::styled(format!("{label:<5}"), theme.footer_style()),
        Span::raw("["),
        Span::styled(fill, Style::default().fg(color)),
        Span::styled(empty, theme.footer_style()),
        Span::raw("] "),
        Span::styled(format!("{:>3}%", (pct * 100.0) as u32), Style::default().fg(color)),
        Span::raw(" "),
        Span::styled(suffix, theme.footer_style()),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

fn gradient(pct: f64, theme: &Theme) -> Color {
    match (pct * 100.0) as u32 {
        0..=49 => theme.usage_low,
        50..=79 => theme.usage_med,
        80..=94 => theme.usage_high,
        _ => theme.usage_critical,
    }
}

fn humanize_mb(mb: u64) -> String {
    if mb >= 1024 * 1024 {
        format!("{:.1}TB", mb as f64 / (1024.0 * 1024.0))
    } else if mb >= 1024 {
        format!("{:.1}GB", mb as f64 / 1024.0)
    } else {
        format!("{}MB", mb)
    }
}
