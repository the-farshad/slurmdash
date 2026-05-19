//! Assist modal (Ctrl+K). Text input on top, response or "thinking…" status
//! below, optional list of proposed commands at the bottom.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use crate::app::AssistDialog;
use crate::tui::theme::Theme;

pub fn render(frame: &mut Frame<'_>, area: Rect, dialog: &AssistDialog, theme: &Theme) {
    let popup = centered_rect(80, 70, area);
    frame.render_widget(Clear, popup);
    let block = Block::default()
        .title(" Assist (Ctrl+K) ")
        .borders(Borders::ALL)
        .border_style(theme.border_style());
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(2),
        ])
        .split(inner);

    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border_style())
        .title(Span::styled(" prompt ", theme.footer_style()));
    let prompt_line = Line::from(vec![
        Span::raw(dialog.input.clone()),
        Span::styled("_", Style::default().fg(theme.accent)),
    ]);
    frame.render_widget(Paragraph::new(prompt_line).block(input_block), layout[0]);

    let body: Vec<Line> = if dialog.in_flight {
        vec![Line::styled(" thinking…", theme.footer_style())]
    } else if let Some(err) = &dialog.error {
        vec![
            Line::styled(" error", Style::default().fg(theme.action_danger)),
            Line::raw(err.clone()),
        ]
    } else if let Some(resp) = &dialog.response {
        let mut out = Vec::new();
        out.push(Line::styled(
            format!(" [{} · {}]", resp.provider, resp.model),
            theme.footer_style(),
        ));
        out.push(Line::raw(""));
        for raw in resp.text.lines() {
            out.push(Line::raw(raw.to_string()));
        }
        if !resp.commands.is_empty() {
            out.push(Line::raw(""));
            out.push(Line::styled(
                " proposed commands (press 1-9 to confirm)",
                theme.header_style(),
            ));
            for (i, cmd) in resp.commands.iter().enumerate().take(9) {
                out.push(Line::from(vec![
                    Span::styled(format!(" {}. ", i + 1), Style::default().fg(theme.accent)),
                    Span::raw(cmd.preview.clone()),
                ]));
            }
        }
        out
    } else {
        vec![Line::styled(
            " type a prompt and press Enter — Esc to close",
            theme.footer_style(),
        )]
    };

    let body_block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border_style());
    frame.render_widget(
        Paragraph::new(body)
            .block(body_block)
            .wrap(Wrap { trim: false }),
        layout[1],
    );

    let hint = Line::from(vec![
        Span::styled("Enter", theme.header_style()),
        Span::raw(" send   "),
        Span::styled("1-9", theme.header_style()),
        Span::raw(" confirm command   "),
        Span::styled("Esc", theme.header_style()),
        Span::raw(" close"),
    ]);
    frame.render_widget(Paragraph::new(hint).style(theme.footer_style()), layout[2]);
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
