//! Inline sparkline strip: three small charts side by side (CPU / GPU / MEM)
//! showing the trailing in-memory `resource_history` from [`AppState`].

use std::collections::VecDeque;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};

use crate::app::ResourceSample;
use crate::tui::theme::Theme;

/// Unicode block characters in increasing height — 8 levels.
const LEVELS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

pub fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    history: &VecDeque<ResourceSample>,
    theme: &Theme,
) {
    let title = format!(" History (last {} samples) ", history.len());
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.border_style());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if history.is_empty() || inner.width < 20 {
        frame.render_widget(
            Paragraph::new(Line::styled(" collecting samples…", theme.footer_style())),
            inner,
        );
        return;
    }

    let has_gpu = history.iter().any(|s| s.has_gpu);
    type Accessor = fn(&ResourceSample) -> f32;
    let columns: Vec<(&str, Accessor)> = if has_gpu {
        vec![
            ("CPU", |s| s.cpu_pct),
            ("GPU", |s| s.gpu_pct),
            ("MEM", |s| s.mem_pct),
        ]
    } else {
        vec![("CPU", |s| s.cpu_pct), ("MEM", |s| s.mem_pct)]
    };

    let pct_pcts: Vec<Constraint> = (0..columns.len())
        .map(|_| Constraint::Percentage(100 / columns.len() as u16))
        .collect();
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(pct_pcts)
        .split(inner);

    for (i, (label, accessor)) in columns.iter().enumerate() {
        let area = chunks[i];
        let current = history.back().map(accessor).unwrap_or(0.0);
        let color = gradient(current as f64, theme);

        let label_w: u16 = 5;
        let pct_w: u16 = 5;
        let spark_w = area.width.saturating_sub(label_w + pct_w + 2) as usize;
        let s = spark(history, *accessor, spark_w);

        let line = Line::from(vec![
            Span::styled(format!("{label:<5}"), theme.footer_style()),
            Span::styled(s, Style::default().fg(color)),
            Span::raw(" "),
            Span::styled(
                format!("{:>3}%", (current * 100.0) as u32),
                Style::default().fg(color),
            ),
        ]);
        frame.render_widget(Paragraph::new(line), area);
    }
}

fn spark(
    history: &VecDeque<ResourceSample>,
    pick: fn(&ResourceSample) -> f32,
    width: usize,
) -> String {
    if width == 0 {
        return String::new();
    }
    // Take the last `width` samples; if we have fewer, pad on the left.
    let samples: Vec<f32> = history.iter().map(pick).collect();
    let start = samples.len().saturating_sub(width);
    let mut out = String::with_capacity(width);
    let pad = width.saturating_sub(samples.len() - start);
    for _ in 0..pad {
        out.push(' ');
    }
    for v in &samples[start..] {
        let idx = ((v.clamp(0.0, 1.0) * 7.0).round() as usize).min(7);
        out.push(LEVELS[idx]);
    }
    out
}

fn gradient(pct: f64, theme: &Theme) -> Color {
    match (pct * 100.0) as u32 {
        0..=49 => theme.usage_low,
        50..=79 => theme.usage_med,
        80..=94 => theme.usage_high,
        _ => theme.usage_critical,
    }
}
