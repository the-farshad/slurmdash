use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Row, Table, TableState};

use crate::app::{AppState, DisplayRow, GroupBy};
use crate::slurm::model::Job;
use crate::tui::theme::Theme;

pub fn render(frame: &mut Frame<'_>, area: Rect, state: &AppState, theme: &Theme) {
    let header_cells = [
        "JOBID",
        "PART",
        "NAME",
        "USER",
        "ST",
        "ELAPSED",
        "LIMIT",
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
        Constraint::Length(10),
        Constraint::Length(10),
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
        Cell::from(j.nodes.to_string()),
        Cell::from(j.reason_or_nodelist.clone()),
    ])
}
