use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState};

use crate::app::{AppState, DisplayRow, FilterMode, GroupBy, GroupSummary};
use crate::slurm::model::Job;
use crate::tui::theme::Theme;

pub fn render(frame: &mut Frame<'_>, area: Rect, state: &AppState, theme: &Theme) {
    // Empty state — render a contextual message instead of a blank table.
    if state.display_rows.is_empty() {
        render_empty(frame, area, state, theme);
        return;
    }

    let header_cells = [
        "JOBID",
        "PART",
        "NAME",
        "USER",
        "ST",
        "ELAPSED",
        "LIMIT",
        "WAIT",
        "N",
        "REASON / NODES",
    ];
    let header = Row::new(
        header_cells
            .into_iter()
            .map(|h| Cell::from(Span::styled(h, Style::default().fg(theme.accent).bold()))),
    );

    let grouped = state.group_by != GroupBy::None;
    let terms = state.current_filter().highlight_terms();
    let rows: Vec<Row> = state
        .display_rows
        .iter()
        .map(|r| match r {
            DisplayRow::Group {
                key,
                collapsed,
                summary,
            } => render_group_row(state.group_by, key, summary, *collapsed, theme),
            DisplayRow::JobIndex(idx) => match state.jobs.get(*idx) {
                Some(j) => render_job_row(j, theme, grouped, &terms),
                None => Row::new(vec![Cell::from("")]),
            },
        })
        .collect();

    let widths = [
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(20),
        Constraint::Length(10),
        Constraint::Length(4),
        Constraint::Length(9),
        Constraint::Length(9),
        Constraint::Length(7),
        Constraint::Length(4),
        Constraint::Fill(1),
    ];

    let table = Table::new(rows, widths)
        .header(header.height(1).bottom_margin(0))
        .block(
            Block::default()
                .borders(Borders::TOP | Borders::BOTTOM)
                .border_style(theme.border_style()),
        )
        .row_highlight_style(Style::default().bg(theme.border))
        .highlight_symbol("▌ ");

    let mut table_state = TableState::default().with_selected(Some(state.selected));
    frame.render_stateful_widget(table, area, &mut table_state);
}

fn render_group_row<'a>(
    kind: GroupBy,
    key: &'a str,
    summary: &'a GroupSummary,
    collapsed: bool,
    theme: &Theme,
) -> Row<'a> {
    let arrow = if collapsed { "▶" } else { "▼" };
    let count = summary.count;

    // Header cell: arrow + key + total count.
    let mut spans = vec![
        Span::styled(
            format!(" {arrow}  "),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{key:<14.14}"),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{count:>3} {}", if count == 1 { "job " } else { "jobs" }),
            Style::default().fg(theme.muted),
        ),
        Span::raw("   "),
    ];

    // Node count.
    if summary.nodes_total > 0 {
        spans.push(Span::styled(
            format!("{:>3}N ", summary.nodes_total),
            Style::default().fg(theme.muted),
        ));
    }

    // State breakdown — only render non-zero buckets, in priority order.
    let state_chips: [(u32, &str, ratatui::style::Color); 6] = [
        (summary.running, "R", theme.running),
        (summary.pending, "PD", theme.pending),
        (summary.completing, "CG", theme.completing),
        (summary.held, "H", theme.held),
        (summary.failed, "F", theme.failed),
        (summary.completed, "CD", theme.completed),
    ];
    for (n, label, color) in state_chips {
        if n == 0 {
            continue;
        }
        spans.push(Span::styled(
            format!("{n}{label} "),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ));
    }

    // Average wait.
    if let Some(w) = summary.avg_wait_seconds {
        spans.push(Span::styled(
            format!("· ~{} wait ", short_dur(w)),
            Style::default().fg(theme.muted),
        ));
    }

    spans.push(Span::styled(
        format!("({})", kind.label()),
        Style::default().fg(theme.border),
    ));

    Row::new(vec![
        Cell::from(ratatui::text::Line::from(spans)),
        Cell::from(""),
        Cell::from(""),
        Cell::from(""),
        Cell::from(""),
        Cell::from(""),
        Cell::from(""),
        Cell::from(""),
        Cell::from(""),
        Cell::from(""),
    ])
}

fn render_job_row<'a>(j: &'a Job, theme: &Theme, indent: bool, terms: &[String]) -> Row<'a> {
    let state_cell = Cell::from(Span::styled(
        j.state.short().to_string(),
        theme.job_state_style(&j.state),
    ));

    let elapsed = j
        .elapsed_seconds
        .map(crate::tui::format::hms)
        .unwrap_or_else(|| "-".into());
    let limit = j
        .time_limit_seconds
        .map(crate::tui::format::hms)
        .unwrap_or_else(|| "-".into());
    let wait = j
        .wait_seconds()
        .map(short_dur)
        .unwrap_or_else(|| "-".into());

    let id_text = if indent {
        format!("  {}", j.job_id)
    } else {
        j.job_id.clone()
    };

    // JOBID gets the accent color so the eye finds rows quickly. If a
    // highlight term matches inside, those substrings still get the
    // higher-contrast highlight style.
    let id_spans = if terms.is_empty() {
        vec![Span::styled(
            id_text.clone(),
            Style::default().fg(theme.accent),
        )]
    } else {
        highlight_spans(&id_text, terms, theme)
    };

    Row::new(vec![
        Cell::from(Line::from(id_spans)),
        Cell::from(Line::from(highlight_spans(&j.partition, terms, theme))),
        Cell::from(Line::from(highlight_spans(&j.name, terms, theme))),
        Cell::from(Line::from(highlight_spans(&j.user, terms, theme))),
        state_cell,
        Cell::from(elapsed),
        Cell::from(limit),
        Cell::from(wait),
        Cell::from(j.nodes.to_string()),
        Cell::from(Line::from(highlight_spans(
            &j.reason_or_nodelist,
            terms,
            theme,
        ))),
    ])
}

/// Split `text` into spans, highlighting any (case-insensitive) substring
/// match of any term in `terms`. Used so the user can see what their `/`
/// filter is hitting inside each row.
fn highlight_spans<'a>(text: &str, terms: &[String], theme: &Theme) -> Vec<Span<'a>> {
    if terms.is_empty() || text.is_empty() {
        return vec![Span::raw(text.to_string())];
    }
    let lower = text.to_lowercase();
    let mut out: Vec<Span<'a>> = Vec::new();
    let mut i = 0usize;
    let bytes = text.as_bytes();
    while i < bytes.len() {
        // Earliest term match starting at or after i.
        let mut best: Option<(usize, usize)> = None;
        for term in terms {
            let t = term.to_lowercase();
            if t.is_empty() {
                continue;
            }
            if let Some(pos) = lower[i..].find(&t) {
                let abs = i + pos;
                let end = abs + t.len();
                match best {
                    None => best = Some((abs, end)),
                    Some((b, _)) if abs < b => best = Some((abs, end)),
                    _ => {}
                }
            }
        }
        match best {
            Some((start, end)) => {
                if start > i {
                    out.push(Span::raw(text[i..start].to_string()));
                }
                out.push(Span::styled(
                    text[start..end].to_string(),
                    Style::default()
                        .fg(theme.bg)
                        .bg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ));
                i = end;
            }
            None => {
                out.push(Span::raw(text[i..].to_string()));
                break;
            }
        }
    }
    out
}

/// Render a contextual empty-state message in place of the table. Explains
/// whether we're still loading, whether a filter is hiding jobs, or whether
/// the queue is genuinely empty for the current `filter:me` / `filter:all`
/// mode.
fn render_empty(frame: &mut Frame<'_>, area: Rect, state: &AppState, theme: &Theme) {
    let block = Block::default()
        .borders(Borders::TOP | Borders::BOTTOM)
        .border_style(theme.border_style());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Pick the most relevant message based on state.
    let (title, hint_lines): (Line, Vec<Line>) = if state.refresh.jobs_in_flight
        && state.all_jobs.is_empty()
        && state.last_error.is_none()
    {
        (
            Line::from(Span::styled(
                "Loading jobs from the cluster…",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )),
            vec![Line::styled(
                "The first squeue refresh is in flight.",
                Style::default().fg(theme.muted),
            )],
        )
    } else if let Some(err) = &state.last_error {
        (
            Line::from(Span::styled(
                "Last refresh failed",
                Style::default()
                    .fg(theme.action_danger)
                    .add_modifier(Modifier::BOLD),
            )),
            vec![
                Line::styled(err.clone(), Style::default().fg(theme.action_danger)),
                Line::raw(""),
                Line::styled(
                    "Press R to retry, or check your SSH config / cluster status.",
                    Style::default().fg(theme.muted),
                ),
            ],
        )
    } else if state.text_filter.is_some() {
        (
            Line::from(Span::styled(
                "No jobs match the active filter",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )),
            vec![
                Line::from(vec![
                    Span::styled("Press ", Style::default().fg(theme.muted)),
                    Span::styled("/", Style::default().fg(theme.accent)),
                    Span::styled(
                        " then Enter on empty to clear, or ",
                        Style::default().fg(theme.muted),
                    ),
                    Span::styled("Esc", Style::default().fg(theme.accent)),
                    Span::styled(" to cancel typing.", Style::default().fg(theme.muted)),
                ]),
                Line::from(vec![
                    Span::styled("Current filter: ", Style::default().fg(theme.muted)),
                    Span::styled(
                        state.text_filter.clone().unwrap_or_default(),
                        Style::default().fg(theme.accent),
                    ),
                ]),
            ],
        )
    } else if matches!(state.filter, FilterMode::Me) {
        (
            Line::from(Span::styled(
                "No jobs of yours in the queue right now",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )),
            vec![
                Line::from(vec![
                    Span::styled("Press ", Style::default().fg(theme.muted)),
                    Span::styled("a", Style::default().fg(theme.accent)),
                    Span::styled(
                        " to switch to filter:all and see everyone's jobs.",
                        Style::default().fg(theme.muted),
                    ),
                ]),
                Line::from(vec![
                    Span::styled(
                        "Submit a job on the cluster (",
                        Style::default().fg(theme.muted),
                    ),
                    Span::styled("sbatch …", Style::default().fg(theme.accent)),
                    Span::styled(
                        ") and it will appear here on the next refresh.",
                        Style::default().fg(theme.muted),
                    ),
                ]),
            ],
        )
    } else {
        (
            Line::from(Span::styled(
                "Queue is empty",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )),
            vec![Line::styled(
                "Nothing pending or running across the whole cluster.",
                Style::default().fg(theme.muted),
            )],
        )
    };

    // Centered vertical block.
    let body_height: u16 = (1 + hint_lines.len() + 1).min(inner.height as usize) as u16;
    let v = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length((inner.height.saturating_sub(body_height)) / 2),
            Constraint::Length(body_height),
            Constraint::Min(0),
        ])
        .split(inner);

    let mut lines = vec![title, Line::raw("")];
    lines.extend(hint_lines);
    let p = Paragraph::new(lines).alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(p, v[1]);
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
