//! Assist modal (Ctrl+K). Text input on top, response or animated
//! "thinking…" status below, optional list of proposed commands at the
//! bottom. Response text is rendered as a small Markdown subset so the
//! model's `**bold**`, `*italic*`, `` `code` ``, ``` ``` ``` fenced
//! blocks, `# headings`, and `-` / `1.` lists land as styled spans
//! instead of raw asterisks.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap};

use crate::app::AssistDialog;
use crate::tui::theme::Theme;

/// Braille spinner frames — drive off [`AppState::frame`] so the dots
/// rotate on every 200 ms redraw tick while the LLM call is in flight.
const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    dialog: &AssistDialog,
    theme: &Theme,
    frame_idx: u64,
) {
    let popup = centered_rect(80, 70, area);
    frame.render_widget(Clear, popup);
    let block = Block::default()
        .title(" Assist (Ctrl+K) ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
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

    // ---- prompt input ----------------------------------------------------
    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.border_style())
        .title(Span::styled(" prompt ", theme.footer_style()));
    let prompt_line = Line::from(vec![
        Span::raw(dialog.input.clone()),
        Span::styled("_", Style::default().fg(theme.accent)),
    ]);
    frame.render_widget(Paragraph::new(prompt_line).block(input_block), layout[0]);

    // ---- body: in_flight spinner, error, or rendered response -----------
    let body: Vec<Line> = if dialog.in_flight {
        let spin = SPINNER[(frame_idx as usize) % SPINNER.len()];
        vec![
            Line::raw(""),
            Line::from(vec![
                Span::styled(
                    format!(" {spin} "),
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "thinking…",
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::raw(""),
            Line::from(Span::styled(
                " sending your prompt + cluster snapshot to the configured LLM.",
                Style::default().fg(theme.muted),
            )),
            Line::from(Span::styled(
                " local models can take ~10s on the first call.",
                Style::default().fg(theme.muted),
            )),
        ]
    } else if let Some(err) = &dialog.error {
        vec![
            Line::styled(" ✘ error", Style::default().fg(theme.action_danger)),
            Line::raw(err.clone()),
        ]
    } else if let Some(resp) = &dialog.response {
        let mut out = Vec::new();
        out.push(Line::from(Span::styled(
            format!(" [{} · {}]", resp.provider, resp.model),
            theme.footer_style(),
        )));
        out.push(Line::raw(""));
        for line in render_markdown(&resp.text, theme) {
            out.push(line);
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
                    Span::styled(cmd.preview.clone(), Style::default().fg(theme.accent)),
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
        .border_type(BorderType::Rounded)
        .border_style(theme.border_style());
    frame.render_widget(
        Paragraph::new(body)
            .block(body_block)
            .wrap(Wrap { trim: false }),
        layout[1],
    );

    // ---- hint footer -----------------------------------------------------
    // The transient copy-status banner replaces the hint line for one
    // redraw cycle (cleared on next keypress) so the user sees that
    // `y` actually did something.
    if let Some(notice) = &dialog.copy_notice {
        let color = if notice.starts_with('✓') {
            theme.usage_low
        } else if notice.starts_with('↗') {
            theme.action_warning
        } else {
            theme.action_danger
        };
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                notice.clone(),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            )))
            .wrap(Wrap { trim: false }),
            layout[2],
        );
    } else {
        let hint = Line::from(vec![
            Span::styled("Enter", theme.header_style()),
            Span::raw(" send · "),
            Span::styled("1-9", theme.header_style()),
            Span::raw(" run command · "),
            Span::styled("y", theme.header_style()),
            Span::raw(" copy response · "),
            Span::styled("Shift+drag", theme.header_style()),
            Span::raw(" select · "),
            Span::styled("Esc", theme.header_style()),
            Span::raw(" close"),
        ]);
        frame.render_widget(Paragraph::new(hint).style(theme.footer_style()), layout[2]);
    }
}

// ---- Markdown-to-spans renderer ---------------------------------------------
//
// The TUI doesn't have a layout-aware Markdown engine, so we handle a
// minimal subset that the LLM actually uses: triple-backtick fenced
// code, inline code, `# / ## / ###` headings, bold (`**`), italic
// (`*` / `_`), unordered (`- `) and ordered (`1. `) lists. Anything
// else passes through verbatim.

fn render_markdown<'a>(text: &str, theme: &Theme) -> Vec<Line<'a>> {
    let mut out: Vec<Line<'a>> = Vec::new();
    let mut in_code_block = false;
    for raw in text.lines() {
        // Toggle fenced-code state on triple-backtick rows.
        if raw.trim_start().starts_with("```") {
            in_code_block = !in_code_block;
            out.push(Line::from(Span::styled(
                raw.to_string(),
                Style::default().fg(theme.border),
            )));
            continue;
        }
        if in_code_block {
            out.push(Line::from(Span::styled(
                format!(" {raw}"),
                Style::default().fg(theme.accent),
            )));
            continue;
        }
        // Heading lines first — the whole line gets accent + bold.
        if let Some(rest) = raw.strip_prefix("### ") {
            out.push(Line::from(Span::styled(
                rest.to_string(),
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )));
            continue;
        }
        if let Some(rest) = raw.strip_prefix("## ") {
            out.push(Line::from(Span::styled(
                rest.to_string(),
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            )));
            continue;
        }
        if let Some(rest) = raw.strip_prefix("# ") {
            out.push(Line::from(Span::styled(
                rest.to_string(),
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            )));
            continue;
        }
        // Bullet lists: replace "- " / "* " with a styled glyph.
        let trimmed = raw.trim_start();
        if let Some(rest) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
        {
            let indent = raw.len() - trimmed.len();
            let mut spans = vec![
                Span::raw(" ".repeat(indent)),
                Span::styled(" • ", Style::default().fg(theme.accent)),
            ];
            spans.extend(inline_spans(rest, theme));
            out.push(Line::from(spans));
            continue;
        }
        // Numbered lists: keep the number, style it.
        if let Some(num_end) = ordered_list_prefix(trimmed) {
            let indent = raw.len() - trimmed.len();
            let (num, rest) = trimmed.split_at(num_end);
            let mut spans = vec![
                Span::raw(" ".repeat(indent)),
                Span::styled(format!(" {num} "), Style::default().fg(theme.accent)),
            ];
            spans.extend(inline_spans(rest, theme));
            out.push(Line::from(spans));
            continue;
        }
        // Horizontal rule.
        if raw.trim() == "---" || raw.trim() == "***" {
            out.push(Line::from(Span::styled(
                "─".repeat(40),
                Style::default().fg(theme.border),
            )));
            continue;
        }
        // Plain prose — apply inline markup.
        out.push(Line::from(inline_spans(raw, theme)));
    }
    out
}

/// Find the position one past the trailing space of an ordered-list
/// marker like "1." / "12.". Returns None if the line doesn't start
/// with one.
fn ordered_list_prefix(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == 0 || i >= bytes.len() {
        return None;
    }
    if bytes[i] != b'.' {
        return None;
    }
    i += 1;
    if i >= bytes.len() || bytes[i] != b' ' {
        return None;
    }
    Some(i + 1)
}

/// Parse `**bold**`, `*italic*`, `` `code` `` inside a single line and
/// emit styled spans. Unmatched markers pass through unchanged.
fn inline_spans<'a>(s: &str, theme: &Theme) -> Vec<Span<'a>> {
    let mut out: Vec<Span<'a>> = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    let mut plain_start = 0;
    while i < bytes.len() {
        // Inline code.
        if bytes[i] == b'`' {
            if let Some(end) = s[i + 1..].find('`') {
                let end_abs = i + 1 + end;
                flush_plain(&s[plain_start..i], &mut out);
                let inner = &s[i + 1..end_abs];
                out.push(Span::styled(
                    inner.to_string(),
                    Style::default().fg(theme.accent).bg(theme.border),
                ));
                i = end_abs + 1;
                plain_start = i;
                continue;
            }
        }
        // Bold (**...**).
        if i + 1 < bytes.len() && &bytes[i..i + 2] == b"**" {
            if let Some(end) = s[i + 2..].find("**") {
                let end_abs = i + 2 + end;
                flush_plain(&s[plain_start..i], &mut out);
                out.push(Span::styled(
                    s[i + 2..end_abs].to_string(),
                    Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
                ));
                i = end_abs + 2;
                plain_start = i;
                continue;
            }
        }
        // Italic (*...*) — single-asterisk, distinguished from `**` by
        // the look-ahead check above failing first.
        if bytes[i] == b'*' {
            if let Some(end) = s[i + 1..].find('*') {
                let end_abs = i + 1 + end;
                flush_plain(&s[plain_start..i], &mut out);
                out.push(Span::styled(
                    s[i + 1..end_abs].to_string(),
                    Style::default().add_modifier(Modifier::ITALIC),
                ));
                i = end_abs + 1;
                plain_start = i;
                continue;
            }
        }
        i += 1;
    }
    if plain_start < s.len() {
        flush_plain(&s[plain_start..], &mut out);
    }
    if out.is_empty() {
        out.push(Span::raw(s.to_string()));
    }
    out
}

fn flush_plain<'a>(s: &str, out: &mut Vec<Span<'a>>) {
    if !s.is_empty() {
        out.push(Span::raw(s.to_string()));
    }
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
