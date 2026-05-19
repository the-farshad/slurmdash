use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::app::AppState;
use crate::history::JobNameStats;
use crate::slurm::state::JobState;
use crate::tui::theme::Theme;

fn humanize_dur(seconds: u64) -> String {
    let h = seconds / 3600;
    let m = (seconds % 3600) / 60;
    if h > 0 {
        format!("{h}h{m:02}m")
    } else {
        format!("{m}m")
    }
}

pub fn render(frame: &mut Frame<'_>, area: Rect, state: &AppState, theme: &Theme) {
    let Some(d) = &state.details else {
        let p = Paragraph::new("(no details loaded)");
        frame.render_widget(p, area);
        return;
    };

    // Reserve a single line for a progress bar when the selected job is
    // running and has both elapsed and time-limit known. Otherwise the
    // details paragraph uses the full area.
    let progress_visible = state
        .selected_job()
        .map(|j| {
            j.state == JobState::Running
                && j.elapsed_seconds.is_some()
                && j.time_limit_seconds.is_some()
        })
        .unwrap_or(false);

    let (progress_area, body_area) = if progress_visible {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(1)])
            .split(area);
        (Some(chunks[0]), chunks[1])
    } else {
        (None, area)
    };

    if let (Some(pa), Some(job)) = (progress_area, state.selected_job()) {
        super::progress::render(
            frame,
            pa,
            "Time",
            job.elapsed_seconds.unwrap_or(0),
            job.time_limit_seconds.unwrap_or(0),
            theme,
        );
    }

    let label_style = theme.header_style();
    let muted_style = theme.footer_style();

    let mut lines = Vec::new();
    macro_rules! kv {
        ($key:expr, $val:expr) => {
            if let Some(v) = $val {
                lines.push(Line::from(vec![
                    Span::styled(format!("  {:<12}", $key), label_style),
                    Span::raw(v.to_string()),
                ]));
            }
        };
    }

    lines.push(Line::from(Span::styled(
        format!(" Job {}", d.job_id),
        theme.header_style(),
    )));
    lines.push(Line::raw(""));

    // Pending-reason explainer: render before the key/value block so users
    // see the action item immediately.
    if let (Some(state_name), Some(reason)) = (&d.state, &d.reason) {
        if state_name.eq_ignore_ascii_case("PENDING") {
            let explained = crate::slurm::reason::explain(reason);
            lines.push(Line::from(Span::styled(
                format!("  Reason  {} — {}", explained.code, explained.summary),
                theme.header_style(),
            )));
            if let Some(suggestion) = explained.suggestion {
                lines.push(Line::from(Span::styled(
                    format!("          {suggestion}"),
                    theme.footer_style(),
                )));
            }
            lines.push(Line::raw(""));
        }
    }
    kv!("Name", d.job_name.as_deref());
    kv!("User", d.user.as_deref());
    kv!("Account", d.account.as_deref());
    kv!("Partition", d.partition.as_deref());
    kv!("QoS", d.qos.as_deref());
    kv!("State", d.state.as_deref());
    kv!("Reason", d.reason.as_deref());
    kv!("Priority", d.priority.as_deref());
    kv!("Dependency", d.dependency.as_deref());
    kv!("Command", d.command.as_deref());
    kv!("WorkDir", d.workdir.as_deref());
    kv!("StdOut", d.stdout.as_deref());
    kv!("StdErr", d.stderr.as_deref());
    kv!("NodeList", d.nodes_alloc.as_deref());
    kv!("ExitCode", d.exit_code.as_deref());

    if let Some(stats) = &state.details_history {
        lines.push(Line::raw(""));
        lines.push(Line::from(Span::styled(
            " History",
            theme.header_style().add_modifier(Modifier::BOLD),
        )));

        // Summary line.
        lines.push(Line::from(vec![
            Span::styled("  runs       ", muted_style),
            Span::styled(format!("{}", stats.runs), Style::default().fg(theme.accent)),
        ]));

        // Outcomes bar — color-coded stacked bar with counts.
        let bar_width: usize = 30;
        outcomes_lines(stats, bar_width, theme, &mut lines);

        // Runtime range — min / p50 / max as horizontal bars on a shared
        // scale. The "suggest --time" value extends the scale so users can
        // visually compare it with the historical max.
        if let (Some(min), Some(p50), Some(max)) = (
            stats.elapsed_min_seconds,
            stats.elapsed_p50_seconds,
            stats.elapsed_max_seconds,
        ) {
            let suggest = max + max / 20;
            let scale = suggest.max(1);
            runtime_range_lines(min, p50, max, suggest, scale, theme, &mut lines);
        }
    }

    if !d.raw.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::from(Span::styled(" raw fields", muted_style)));
        for (k, v) in &d.raw {
            lines.push(Line::from(vec![
                Span::styled(format!("  {k}="), muted_style),
                Span::raw(v.clone()),
            ]));
        }
    }

    let block = Block::default()
        .borders(Borders::TOP | Borders::BOTTOM)
        .border_style(theme.border_style());
    frame.render_widget(
        Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false }),
        body_area,
    );
}

/// One "Outcomes" line + a stacked bar segmented into completed / failed /
/// timeout / cancelled / other categories with their respective colors.
fn outcomes_lines<'a>(
    stats: &JobNameStats,
    bar_width: usize,
    theme: &Theme,
    lines: &mut Vec<Line<'a>>,
) {
    let cd = stats.completions;
    let f = stats.failures;
    let to = stats.timeouts;
    let ca = stats.cancellations;
    let total = stats.runs.max(1);
    let other = stats.runs.saturating_sub(cd + f + to + ca);

    let segments: [(u32, &str, ratatui::style::Color); 5] = [
        (cd, "CD", theme.completed),
        (f, "F", theme.failed),
        (to, "TO", theme.failed),
        (ca, "CA", theme.cancelled),
        (other, "•", theme.muted),
    ];

    let mut spans: Vec<Span<'_>> = Vec::with_capacity(16);
    spans.push(Span::styled(
        "  outcomes   ",
        Style::default().fg(theme.muted),
    ));
    spans.push(Span::raw("["));
    let mut used = 0usize;
    for (i, (count, _, color)) in segments.iter().enumerate() {
        let mut w = ((*count as f64 / total as f64) * bar_width as f64).round() as usize;
        // Make sure the last non-zero segment fills any rounding remainder.
        if i == segments.len() - 1 {
            w = bar_width.saturating_sub(used);
        }
        if w == 0 {
            continue;
        }
        spans.push(Span::styled("▰".repeat(w), Style::default().fg(*color)));
        used += w;
    }
    if used < bar_width {
        spans.push(Span::styled(
            "▱".repeat(bar_width - used),
            Style::default().fg(theme.border),
        ));
    }
    spans.push(Span::raw("]"));
    lines.push(Line::from(spans));

    // Legend with counts and colored labels.
    let mut legend: Vec<Span<'_>> = vec![Span::styled(
        "             ",
        Style::default().fg(theme.muted),
    )];
    let labels: [(u32, &str, ratatui::style::Color); 4] = [
        (cd, "CD completed", theme.completed),
        (f, "F failed", theme.failed),
        (to, "TO timeout", theme.failed),
        (ca, "CA cancelled", theme.cancelled),
    ];
    let mut first = true;
    for (count, lbl, color) in labels {
        if count == 0 {
            continue;
        }
        if !first {
            legend.push(Span::styled(" · ", Style::default().fg(theme.muted)));
        }
        legend.push(Span::styled(
            format!("{count} "),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ));
        legend.push(Span::styled(lbl, Style::default().fg(theme.muted)));
        first = false;
    }
    lines.push(Line::from(legend));
}

/// Four horizontal bars on a shared scale showing min / p50 / max / suggest
/// runtimes. The suggested padding sits visually relative to the historical
/// max so users see how much headroom it adds.
fn runtime_range_lines<'a>(
    min: u64,
    p50: u64,
    max: u64,
    suggest: u64,
    scale: u64,
    theme: &Theme,
    lines: &mut Vec<Line<'a>>,
) {
    let bar_w: usize = 30;
    let render_bar = |value: u64, label: &str, color: ratatui::style::Color| -> Line<'a> {
        let filled = ((value as f64 / scale as f64) * bar_w as f64).round() as usize;
        let fill = "▰".repeat(filled.min(bar_w));
        let empty = "▱".repeat(bar_w.saturating_sub(filled));
        Line::from(vec![
            Span::styled(format!("  {label:<10}"), Style::default().fg(theme.muted)),
            Span::styled(fill, Style::default().fg(color)),
            Span::styled(empty, Style::default().fg(theme.border)),
            Span::styled(
                format!(" {}", humanize_dur(value)),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
        ])
    };

    lines.push(Line::from(Span::styled(
        "  runtime",
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(render_bar(min, "min", theme.usage_low));
    lines.push(render_bar(p50, "p50", theme.usage_med));
    lines.push(render_bar(max, "max", theme.usage_high));
    lines.push(render_bar(suggest, "suggest", theme.accent));
}
