//! Single-line totals strip rendered directly below the jobs table.
//! Aggregates the currently-visible (filtered) job set so the user can
//! see at a glance what their filter actually selected — total count,
//! state breakdown, sum of nodes / GPUs / memory, max time-limit, and
//! mean wait. Recomputed every redraw; the per-job pass is cheap.

use std::collections::BTreeMap;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::AppState;
use crate::tui::theme::Theme;

pub fn render(frame: &mut Frame<'_>, area: Rect, state: &AppState, theme: &Theme) {
    let jobs = &state.jobs;
    let mut spans: Vec<Span<'_>> = Vec::new();
    let sep = || Span::styled("  ·  ", Style::default().fg(theme.border));
    let muted_label = |text: String| Span::styled(text, Style::default().fg(theme.muted));

    // "Σ N jobs" — always shown, even when zero, so the user sees a
    // filter that hides everything.
    spans.push(Span::styled("Σ ", Style::default().fg(theme.muted)));
    spans.push(Span::styled(
        format!(
            "{} job{}",
            jobs.len(),
            if jobs.len() == 1 { "" } else { "s" }
        ),
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD),
    ));

    // Filter chip — "filter:me + /alice/ (12 of 87)" so the user is
    // never confused about WHY the count is what it is.
    spans.push(sep());
    spans.push(muted_label(format!("filter:{}", state.filter.label())));
    if let Some(text) = &state.text_filter {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            format!("/{text}/"),
            Style::default().fg(theme.accent),
        ));
        spans.push(Span::styled(
            format!("  {} of {}", state.jobs.len(), state.all_jobs.len()),
            Style::default().fg(theme.muted),
        ));
    }

    if jobs.is_empty() {
        frame.render_widget(Paragraph::new(Line::from(spans)), area);
        return;
    }

    // Walk the filtered job set once.
    let mut by_state: BTreeMap<&str, u32> = BTreeMap::new();
    let mut nodes: u32 = 0;
    let mut gpus: u32 = 0;
    let mut mem_mb_total: u64 = 0;
    let mut mem_n: u32 = 0;
    let mut wait_sum: u64 = 0;
    let mut wait_n: u32 = 0;
    let mut limit_max: u64 = 0;
    let mut elapsed_total: u64 = 0;
    for j in jobs {
        *by_state.entry(j.state.short()).or_insert(0) += 1;
        nodes = nodes.saturating_add(j.nodes);
        gpus = gpus.saturating_add(j.gpus());
        if let Some(m) = j.min_mem_mb {
            mem_mb_total = mem_mb_total.saturating_add(m);
            mem_n += 1;
        }
        if let Some(w) = j.wait_seconds() {
            wait_sum = wait_sum.saturating_add(w);
            wait_n += 1;
        }
        if let Some(l) = j.time_limit_seconds {
            limit_max = limit_max.max(l);
        }
        if let Some(e) = j.elapsed_seconds {
            elapsed_total = elapsed_total.saturating_add(e);
        }
    }

    if !by_state.is_empty() {
        spans.push(sep());
        let mut first = true;
        for (st, n) in &by_state {
            if !first {
                spans.push(Span::raw(" "));
            }
            let color = match *st {
                "R" => theme.running,
                "PD" => theme.pending,
                "CG" => theme.completing,
                "CD" => theme.completed,
                "H" => theme.held,
                "F" | "TO" | "NF" | "BF" | "DL" | "OOM" => theme.failed,
                "CA" => theme.cancelled,
                _ => theme.fg,
            };
            spans.push(Span::styled(
                format!("{n}{st}"),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ));
            first = false;
        }
    }

    spans.push(sep());
    spans.push(muted_label("Σnodes ".into()));
    spans.push(Span::styled(
        format!("{nodes}"),
        Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
    ));

    if gpus > 0 {
        spans.push(sep());
        spans.push(muted_label("ΣGPUs ".into()));
        spans.push(Span::styled(
            format!("{gpus}"),
            Style::default()
                .fg(theme.action_normal)
                .add_modifier(Modifier::BOLD),
        ));
    }

    if mem_n > 0 {
        spans.push(sep());
        spans.push(muted_label("Σmem ".into()));
        spans.push(Span::styled(
            humanize_mb(mem_mb_total),
            Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
        ));
    }

    if elapsed_total > 0 {
        spans.push(sep());
        spans.push(muted_label("Σelapsed ".into()));
        spans.push(Span::styled(
            short_dur(elapsed_total),
            Style::default()
                .fg(theme.action_normal)
                .add_modifier(Modifier::BOLD),
        ));
    }

    if limit_max > 0 {
        spans.push(sep());
        spans.push(muted_label("max limit ".into()));
        spans.push(Span::styled(
            short_dur(limit_max),
            Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
        ));
    }

    if wait_n > 0 {
        spans.push(sep());
        spans.push(muted_label("avg wait ".into()));
        spans.push(Span::styled(
            short_dur(wait_sum / wait_n as u64),
            Style::default()
                .fg(theme.action_warning)
                .add_modifier(Modifier::BOLD),
        ));
    }

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn humanize_mb(mb: u64) -> String {
    if mb >= 1024 * 1024 {
        format!("{:.1}TB", mb as f64 / 1024.0 / 1024.0)
    } else if mb >= 1024 {
        format!("{:.1}GB", mb as f64 / 1024.0)
    } else {
        format!("{mb}MB")
    }
}

fn short_dur(s: u64) -> String {
    if s < 60 {
        format!("{s}s")
    } else if s < 3600 {
        format!("{}m", s / 60)
    } else if s < 86_400 {
        let h = s / 3600;
        let m = (s % 3600) / 60;
        if m == 0 {
            format!("{h}h")
        } else {
            format!("{h}h{m}m")
        }
    } else {
        format!("{}d", s / 86_400)
    }
}
