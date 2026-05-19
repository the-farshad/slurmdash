use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Row, Table, TableState};

use crate::app::AppState;
use crate::slurm::model::Job;
use crate::tui::theme::Theme;

pub fn render(frame: &mut Frame<'_>, area: Rect, state: &AppState, theme: &Theme) {
    let header_cells = [
        "JOBID", "PART", "NAME", "USER", "ST", "ELAPSED", "LIMIT", "N", "REASON / NODES",
    ];
    let header = Row::new(header_cells.into_iter().map(|h| {
        Cell::from(Span::styled(
            h,
            Style::default().fg(theme.accent).bold(),
        ))
    }));

    let rows: Vec<Row> = state
        .jobs
        .iter()
        .map(|j| render_row(j, theme))
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

fn render_row<'a>(j: &'a Job, theme: &Theme) -> Row<'a> {
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

    Row::new(vec![
        Cell::from(j.job_id.clone()),
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
