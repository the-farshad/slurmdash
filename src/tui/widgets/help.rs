use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::tui::theme::Theme;

pub fn render(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let popup = centered_rect(60, 70, area);
    frame.render_widget(Clear, popup);

    let lines = vec![
        Line::from(Span::styled("Keyboard", theme.header_style())),
        Line::raw(""),
        Line::raw("Navigation"),
        Line::raw("  ↑ k        select previous job"),
        Line::raw("  ↓ j        select next job"),
        Line::raw("  g Home     jump to top"),
        Line::raw("  G End      jump to bottom"),
        Line::raw("  Enter d    open job details"),
        Line::raw("  Esc        back to job list"),
        Line::raw(""),
        Line::raw("Actions   (each opens a confirm modal)"),
        Line::raw("  c          scancel"),
        Line::raw("  h          scontrol hold"),
        Line::raw("  u          scontrol release"),
        Line::raw("  Q          scontrol requeue"),
        Line::raw("  Enter / y  confirm"),
        Line::raw("  Esc / n    cancel"),
        Line::raw(""),
        Line::raw("Assist (LLM)"),
        Line::raw("  Ctrl-K     open prompt assistant"),
        Line::raw("  1..9       confirm proposed command"),
        Line::raw(""),
        Line::raw("Other"),
        Line::raw("  R r        refresh now"),
        Line::raw("  1 / 2      dashboard / jobs view"),
        Line::raw("  ?          this help"),
        Line::raw("  q          quit"),
        Line::raw("  Ctrl-C     quit immediately"),
        Line::raw(""),
        Line::styled("  press any key to dismiss", theme.footer_style()),
    ];

    let block = Block::default()
        .title(" Help ")
        .borders(Borders::ALL)
        .border_style(theme.border_style());
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
