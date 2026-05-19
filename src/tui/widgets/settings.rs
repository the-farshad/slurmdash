//! Settings panel — shows current LLM/theme config and lets the user
//! probe-test the configured model.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};

use crate::app::AppState;
use crate::tui::theme::Theme;

pub fn render(frame: &mut Frame<'_>, area: Rect, state: &AppState, theme: &Theme) {
    let block = Block::default()
        .title(" Settings ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.border_style());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // section title: LLM
            Constraint::Length(6), // 4 editable LLM rows + nav hint
            Constraint::Length(1), // section title: Theme
            Constraint::Length(2), // theme + cycle hint
            Constraint::Length(1), // section title: Web UI
            Constraint::Length(5), // web UI status rows
            Constraint::Length(1), // section title: Test
            Constraint::Min(4),    // test result area
        ])
        .split(inner);

    let header = |s: &'static str| -> Line<'static> {
        Line::from(Span::styled(
            s,
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ))
    };
    let kv = |k: &str, v: String, color: ratatui::style::Color| -> Line {
        Line::from(vec![
            Span::styled(format!("    {:<10}", k), Style::default().fg(theme.muted)),
            Span::styled(v, Style::default().fg(color)),
        ])
    };

    let anthropic_key_set = std::env::var("ANTHROPIC_API_KEY").is_ok();

    frame.render_widget(Paragraph::new(header(" LLM assistant")), chunks[0]);

    // Render the 4 editable LLM rows. The currently-selected row gets a
    // `▎` indicator; if `edit_buffer` is Some, the selected row shows
    // the live input buffer with a trailing cursor block.
    let llm = &state.settings.llm;
    let cursor = state.settings.cursor;
    let editing = state.settings.edit_buffer.is_some();
    let mut llm_lines: Vec<Line> = Vec::new();
    for i in 0..crate::app::LlmConfig::FIELDS {
        let is_selected = i == cursor;
        let prefix = if is_selected { "▎ " } else { "  " };
        let label = crate::app::LlmConfig::field_label(i);
        let value: String = if is_selected && editing {
            format!("{}_", state.settings.edit_buffer.as_deref().unwrap_or(""))
        } else {
            llm.field_value(i).to_string()
        };
        let value_color = if is_selected && editing {
            theme.action_warning
        } else if is_selected {
            theme.accent
        } else {
            theme.fg
        };
        let prefix_color = if is_selected { theme.accent } else { theme.bg };
        llm_lines.push(Line::from(vec![
            Span::styled(
                prefix,
                Style::default()
                    .fg(prefix_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("{label:<16}"), Style::default().fg(theme.muted)),
            Span::styled(value, Style::default().fg(value_color)),
        ]));
    }
    let hint_text = if editing {
        "    Enter commit · Esc cancel · Backspace delete"
    } else {
        "    ↑↓ select · e or Enter edit · t test · API_KEY stays in env only"
    };
    llm_lines.push(Line::from(Span::styled(
        format!(
            "{hint_text}{}",
            if anthropic_key_set {
                " · ANTHROPIC_API_KEY set"
            } else {
                ""
            }
        ),
        Style::default().fg(theme.muted),
    )));
    frame.render_widget(Paragraph::new(llm_lines), chunks[1]);

    frame.render_widget(Paragraph::new(header(" Theme")), chunks[2]);
    let theme_lines = vec![
        kv("active", state.theme_name.clone(), theme.accent),
        Line::from(Span::styled(
            "    (press T anywhere in the TUI to cycle dark / light / high-contrast / colorblind-safe)",
            Style::default().fg(theme.muted),
        )),
    ];
    frame.render_widget(Paragraph::new(theme_lines), chunks[3]);

    // ---- Web UI section ------------------------------------------------
    frame.render_widget(Paragraph::new(header(" Web UI")), chunks[4]);
    let web_lines: Vec<Line> = if let Some(info) = &state.web.running {
        vec![
            kv("status", "running".into(), theme.usage_low),
            kv("url", info.url.clone(), theme.accent),
            kv("listen", info.addr.clone(), theme.fg),
            kv(
                "mode",
                if info.readonly {
                    "readonly".into()
                } else {
                    "read/write".into()
                },
                theme.fg,
            ),
            Line::from(Span::styled(
                "    copy the url above into a browser to access the dashboard",
                Style::default().fg(theme.muted),
            )),
        ]
    } else if state.web.starting {
        vec![Line::from(Span::styled(
            "    binding loopback port…",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ))]
    } else if let Some(err) = &state.web.last_error {
        vec![
            Line::from(Span::styled(
                "    ✘ failed to start",
                Style::default()
                    .fg(theme.action_danger)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                format!("    {err}"),
                Style::default().fg(theme.action_danger),
            )),
            Line::from(Span::styled(
                "    (port may already be in use — press w again to retry)",
                Style::default().fg(theme.muted),
            )),
        ]
    } else {
        vec![
            Line::from(Span::styled(
                "    not running — press w to start on a loopback port",
                Style::default().fg(theme.muted),
            )),
            Line::from(Span::styled(
                "    (or run `slurmdash --host <alias> web --port 8080` in another shell)",
                Style::default().fg(theme.muted),
            )),
        ]
    };
    frame.render_widget(
        Paragraph::new(web_lines).wrap(Wrap { trim: false }),
        chunks[5],
    );

    frame.render_widget(Paragraph::new(header(" Test")), chunks[6]);
    let mut test_lines: Vec<Line> = Vec::new();
    if state.settings.test_in_flight {
        // Animate the same braille spinner the header uses so the user
        // sees the probe is alive — the LLM call can take 10+ seconds
        // against a cold local model.
        const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let frame_idx = (state.frame as usize) % SPINNER.len();
        test_lines.push(Line::from(vec![
            Span::styled("    ", Style::default()),
            Span::styled(
                SPINNER[frame_idx],
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                " sending probe to the configured model…",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        test_lines.push(Line::from(Span::styled(
            "      (don't close the view — response will pop in here)",
            Style::default().fg(theme.muted),
        )));
    } else if let Some(err) = &state.settings.test_error {
        test_lines.push(Line::from(Span::styled(
            "    ✘ probe failed",
            Style::default()
                .fg(theme.action_danger)
                .add_modifier(Modifier::BOLD),
        )));
        test_lines.push(Line::from(Span::styled(
            format!("    {err}"),
            Style::default().fg(theme.action_danger),
        )));
    } else if let Some(out) = &state.settings.test_result {
        test_lines.push(Line::from(Span::styled(
            "    ✓ probe ok — model responded",
            Style::default()
                .fg(theme.usage_low)
                .add_modifier(Modifier::BOLD),
        )));
        for raw in out.lines().take(8) {
            test_lines.push(Line::from(Span::styled(
                format!("      {raw}"),
                Style::default().fg(theme.fg),
            )));
        }
    } else {
        test_lines.push(Line::from(Span::styled(
            "    press t to send a one-line probe to the configured model",
            Style::default().fg(theme.muted),
        )));
    }
    frame.render_widget(
        Paragraph::new(test_lines).wrap(Wrap { trim: false }),
        chunks[7],
    );
}
