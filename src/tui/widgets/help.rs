use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};

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
        Line::raw("Filter / grouping"),
        Line::raw("  a          toggle between your jobs (--me) and all jobs"),
        Line::raw("  /          search jobs by id / name / user / partition / reason"),
        Line::raw("             field tokens:  user:alice  partition:gpu  state:R"),
        Line::raw("                            name:train  id:12345  reason:Resources"),
        Line::raw("             resource tokens:  gpu  cpu  m:>16G  m:<32G"),
        Line::raw("  Tab        cycle grouping: flat / user / partition / state"),
        Line::raw("  Enter      on a group header: collapse / expand"),
        Line::raw("  PgUp/PgDn  move selection by a page (10 rows)"),
        Line::raw("  Ctrl+U     half-page up    (5 rows, vim-style)"),
        Line::raw("  Ctrl+D     half-page down  (5 rows, vim-style)"),
        Line::raw("  Ctrl+B     full-page up    (10 rows, less-style)"),
        Line::raw("  Ctrl+F     full-page down  (10 rows, less-style)"),
        Line::raw(""),
        Line::raw("Actions   (each opens a confirm modal)"),
        Line::raw("  c          scancel"),
        Line::raw("  h          scontrol hold"),
        Line::raw("  u          scontrol release"),
        Line::raw("  Q          scontrol requeue"),
        Line::raw("  Enter / y  confirm"),
        Line::raw("  Esc / n    cancel"),
        Line::raw(""),
        Line::raw("Theme"),
        Line::raw("  T          cycle dark / light / high-contrast / colorblind-safe"),
        Line::raw(""),
        Line::raw("Assist (LLM)"),
        Line::raw("  Ctrl+K     open prompt assistant"),
        Line::raw("  1..9       confirm proposed command"),
        Line::raw(""),
        Line::raw("Other"),
        Line::raw("  R r        refresh now"),
        Line::raw("  1 / 2 / 3  dashboard / jobs / statistics"),
        Line::raw("  ?          this help"),
        Line::raw("  q          quit"),
        Line::raw("  Ctrl+C     quit immediately"),
        Line::raw(""),
        Line::styled("  press any key to dismiss", theme.footer_style()),
    ];

    let block = Block::default()
        .title(" Help ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
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
