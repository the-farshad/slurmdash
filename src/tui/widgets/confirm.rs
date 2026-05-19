use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};

use crate::actions::ActionKind;
use crate::app::Confirm;
use crate::tui::theme::Theme;

pub fn render(frame: &mut Frame<'_>, area: Rect, confirm: &Confirm, theme: &Theme) {
    let popup = centered_rect(70, 25, area);
    frame.render_widget(Clear, popup);

    let title_color = match confirm.kind {
        ActionKind::Cancel => theme.action_danger,
        ActionKind::Hold => theme.action_warning,
        ActionKind::Release | ActionKind::Requeue => theme.action_normal,
    };

    let lines = vec![
        Line::from(Span::styled(
            format!("{} job {}", confirm.kind.label(), confirm.job_id),
            Style::default().fg(title_color),
        )),
        Line::raw(""),
        Line::from(vec![
            Span::styled("$ ", theme.footer_style()),
            Span::raw(confirm.preview.clone()),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled("Enter", theme.header_style()),
            Span::raw(" / "),
            Span::styled("y", theme.header_style()),
            Span::raw(" to confirm    "),
            Span::styled("Esc", theme.header_style()),
            Span::raw(" / "),
            Span::styled("n", theme.header_style()),
            Span::raw(" to cancel"),
        ]),
    ];

    let block = Block::default()
        .title(" Confirm ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(title_color));
    frame.render_widget(Paragraph::new(lines).block(block), popup);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let v = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(v[1])[1]
}
