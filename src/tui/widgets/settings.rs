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
            Constraint::Length(5), // LLM rows
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

    let provider = std::env::var("SLURMDASH_LLM_PROVIDER").unwrap_or_else(|_| "ollama".into());
    let ollama_host =
        std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434".into());
    let ollama_model = std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "llama3.2".into());
    let anthropic_model =
        std::env::var("ANTHROPIC_MODEL").unwrap_or_else(|_| "claude-sonnet-4-6".into());
    let anthropic_key_set = std::env::var("ANTHROPIC_API_KEY").is_ok();

    frame.render_widget(Paragraph::new(header(" LLM assistant")), chunks[0]);
    let mut llm_lines: Vec<Line> = Vec::new();
    llm_lines.push(kv("provider", provider.clone(), theme.accent));
    if provider == "ollama" {
        llm_lines.push(kv("host", ollama_host, theme.fg));
        llm_lines.push(kv("model", ollama_model, theme.fg));
    } else {
        llm_lines.push(kv("model", anthropic_model, theme.fg));
        llm_lines.push(kv(
            "api_key",
            if anthropic_key_set {
                "set (hidden)".into()
            } else {
                "NOT SET — export ANTHROPIC_API_KEY".into()
            },
            if anthropic_key_set {
                theme.usage_low
            } else {
                theme.action_danger
            },
        ));
    }
    llm_lines.push(Line::from(Span::styled(
        "    (override via SLURMDASH_LLM_PROVIDER / OLLAMA_HOST / OLLAMA_MODEL env vars)",
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
        test_lines.push(Line::from(Span::styled(
            "    sending probe to the configured model…",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
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
