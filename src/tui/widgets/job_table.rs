use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState};

use crate::app::{AppState, DisplayRow, FilterMode, GroupBy};
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
    let rows: Vec<Row> = state
        .display_rows
        .iter()
        .map(|r| match r {
            DisplayRow::Group {
                key,
                count,
                collapsed,
            } => render_group_row(state.group_by, key, *count, *collapsed, theme),
            DisplayRow::JobIndex(idx) => match state.jobs.get(*idx) {
                Some(j) => render_job_row(j, theme, grouped),
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
    count: u32,
    collapsed: bool,
    theme: &Theme,
) -> Row<'a> {
    let arrow = if collapsed { "▶" } else { "▼" };
    let label = format!(
        " {arrow}  {key}    {count} {} ({})",
        if count == 1 { "job" } else { "jobs" },
        kind.label()
    );
    let cell = Cell::from(Span::styled(
        label,
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD),
    ));
    Row::new(vec![
        cell,
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

fn render_job_row<'a>(j: &'a Job, theme: &Theme, indent: bool) -> Row<'a> {
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

    let id_cell = if indent {
        Cell::from(format!("  {}", j.job_id))
    } else {
        Cell::from(j.job_id.clone())
    };

    Row::new(vec![
        id_cell,
        Cell::from(j.partition.clone()),
        Cell::from(Line::from(Span::raw(j.name.clone()))),
        Cell::from(j.user.clone()),
        state_cell,
        Cell::from(elapsed),
        Cell::from(limit),
        Cell::from(wait),
        Cell::from(j.nodes.to_string()),
        Cell::from(j.reason_or_nodelist.clone()),
    ])
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
