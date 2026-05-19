//! Partition cards. One row per partition with CPU/GPU/memory bars.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::slurm::model::Partition;
use crate::tui::theme::Theme;

pub fn render(frame: &mut Frame<'_>, area: Rect, partitions: &[Partition], theme: &Theme) {
    let block = Block::default()
        .title(" Partitions ")
        .borders(Borders::ALL)
        .border_style(theme.border_style());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if partitions.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::styled(
                " sinfo did not return any partitions",
                theme.footer_style(),
            )),
            inner,
        );
        return;
    }

    let rows = partitions.len().min(inner.height as usize);
    if rows == 0 {
        return;
    }
    let constraints = vec![Constraint::Length(1); rows];
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    for (i, p) in partitions.iter().take(rows).enumerate() {
        render_row(frame, chunks[i], p, theme);
    }
}

fn render_row(frame: &mut Frame<'_>, area: Rect, p: &Partition, theme: &Theme) {
    let has_gpu = p.gpus_per_node.is_some() && p.gpus_per_node.unwrap() > 0;
    let segments = if has_gpu { 3 } else { 2 };

    let name_w: u16 = 14;
    let nodes_w: u16 = 12;
    let label_width = name_w + nodes_w + 4;
    let bar_total = area.width.saturating_sub(label_width) as usize;
    let bar_width = (bar_total / segments).max(8);

    let cpu_pct = p.cpus.pct_allocated();
    let gpu_pct = if has_gpu {
        let total = p.gpus_per_node.unwrap_or(0) * p.nodes.total;
        let alloc = p.gpus_per_node.unwrap_or(0) * p.nodes.allocated;
        if total == 0 {
            0.0
        } else {
            alloc as f64 / total as f64
        }
    } else {
        0.0
    };
    let mem_pct = if let Some(per) = p.memory_mb_per_node {
        let total = per * p.nodes.total as u64;
        let alloc = per * p.nodes.allocated as u64;
        if total == 0 {
            0.0
        } else {
            alloc as f64 / total as f64
        }
    } else {
        0.0
    };

    let nodes = format!("{}/{} nodes", p.nodes.allocated, p.nodes.total);
    let mut spans = vec![
        Span::styled(
            format!("{:<width$}", p.name, width = name_w as usize),
            theme.header_style(),
        ),
        Span::raw("  "),
    ];

    spans.extend(bar_segment("cpu", cpu_pct, bar_width, theme));
    spans.push(Span::raw(" "));
    if has_gpu {
        spans.extend(bar_segment("gpu", gpu_pct, bar_width, theme));
        spans.push(Span::raw(" "));
    }
    spans.extend(bar_segment("mem", mem_pct, bar_width, theme));
    spans.push(Span::raw("  "));
    spans.push(Span::styled(nodes, theme.footer_style()));

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn bar_segment<'a>(label: &'a str, pct: f64, bar_w: usize, theme: &Theme) -> Vec<Span<'a>> {
    let color = gradient(pct, theme);
    let inner = bar_w.saturating_sub(label.len() + 6);
    let filled = (pct * inner as f64).round() as usize;
    let fill: String = "█".repeat(filled);
    let empty: String = "░".repeat(inner.saturating_sub(filled));
    vec![
        Span::styled(format!("{label} "), theme.footer_style()),
        Span::styled(fill, Style::default().fg(color)),
        Span::styled(empty, theme.footer_style()),
        Span::styled(
            format!(" {:>3}%", (pct * 100.0) as u32),
            Style::default().fg(color),
        ),
    ]
}

fn gradient(pct: f64, theme: &Theme) -> Color {
    match (pct * 100.0) as u32 {
        0..=49 => theme.usage_low,
        50..=79 => theme.usage_med,
        80..=94 => theme.usage_high,
        _ => theme.usage_critical,
    }
}
