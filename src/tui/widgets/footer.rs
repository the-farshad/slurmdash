//! Two-line footer.
//!
//! Top line: colored key-hints. Each hint is a `[key] label` pair —
//! key in accent / danger / special color (bold), label muted. Errors
//! replace this row and clear on the next successful refresh.
//!
//! Bottom line: live status strip — view name on the left, then chips
//! for filter / sort / group / cluster / refresh-status. Gives the
//! user a stable "where am I and what's running" anchor at all times.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use ratatui::widgets::Wrap;

use crate::app::{AppState, View};
use crate::tui::theme::Theme;

#[derive(Copy, Clone)]
enum HintKind {
    Normal,
    Danger,
    Special,
}

#[derive(Copy, Clone)]
struct Hint {
    key: &'static str,
    label: &'static str,
    kind: HintKind,
}

pub fn render(frame: &mut Frame<'_>, area: Rect, theme: &Theme, state: &AppState) {
    // Top line gets the hints (or the error banner if last_error is set).
    // Bottom line gets the live status chip strip. Splitting the footer
    // gives ample horizontal room for both without truncation.
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(area);

    if let Some(err) = &state.last_error {
        let line = Line::from(vec![
            Span::styled(
                " ✘ ",
                Style::default()
                    .fg(theme.action_danger)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(err.clone(), Style::default().fg(theme.action_danger)),
            Span::raw("  "),
            Span::styled(
                "(auto-clears on next refresh)",
                Style::default().fg(theme.muted),
            ),
        ]);
        frame.render_widget(Paragraph::new(line), chunks[0]);
    } else {
        let hints: &[Hint] = match state.view {
            View::Dashboard | View::Jobs | View::Statistics => &JOBS_HINTS,
            View::Details => &DETAILS_HINTS,
            View::Logs => &LOGS_HINTS,
            View::Settings => &SETTINGS_HINTS,
        };
        let mut spans = Vec::with_capacity(hints.len() * 4 + 1);
        for (i, h) in hints.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(" · ", Style::default().fg(theme.border)));
            }
            let key_color = match h.kind {
                HintKind::Normal => theme.accent,
                HintKind::Danger => theme.action_danger,
                HintKind::Special => theme.cancelled,
            };
            spans.push(Span::styled(
                h.key,
                Style::default().fg(key_color).add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::raw(" "));
            spans.push(Span::styled(h.label, Style::default().fg(theme.muted)));
        }
        frame.render_widget(
            Paragraph::new(Line::from(spans)).wrap(Wrap { trim: false }),
            chunks[0],
        );
    }

    // Bottom status strip — view chip + filter/sort/group/refresh chips.
    let view_label = match state.view {
        View::Dashboard => "DASH",
        View::Jobs => "JOBS",
        View::Statistics => "STATS",
        View::Details => "DETAILS",
        View::Logs => "LOGS",
        View::Settings => "SETTINGS",
    };
    let mut bottom: Vec<Span<'_>> = vec![
        Span::styled(
            format!(" {view_label} "),
            Style::default()
                .fg(theme.bg)
                .bg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
    ];
    let chip = |label: &str, value: String, color: ratatui::style::Color| -> Vec<Span<'_>> {
        vec![
            Span::styled(format!("{label}:"), Style::default().fg(theme.muted)),
            Span::styled(
                value,
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled("  ", Style::default()),
        ]
    };

    bottom.extend(chip("filter", state.filter.label(), theme.accent));
    bottom.extend(chip(
        "sort",
        format!(
            "{}{}",
            state.sort.key.label(),
            if state.sort.reverse { "↓" } else { "↑" }
        ),
        theme.accent,
    ));
    bottom.extend(chip(
        "group",
        state.group_by.label().to_string(),
        theme.accent,
    ));
    let job_count = if state.text_filter.is_some() && state.jobs.len() != state.all_jobs.len() {
        format!("{}/{}", state.jobs.len(), state.all_jobs.len())
    } else {
        format!("{}", state.all_jobs.len().max(state.jobs.len()))
    };
    bottom.extend(chip("jobs", job_count, theme.fg));
    let refreshing = state.refresh.jobs_in_flight || state.refresh.sinfo_in_flight;
    if refreshing {
        bottom.push(Span::styled(
            "● refreshing",
            Style::default()
                .fg(theme.pending)
                .add_modifier(Modifier::BOLD),
        ));
    } else {
        bottom.push(Span::styled(
            "● ready",
            Style::default()
                .fg(theme.running)
                .add_modifier(Modifier::BOLD),
        ));
    }

    frame.render_widget(
        Paragraph::new(Line::from(bottom)).wrap(Wrap { trim: false }),
        chunks[1],
    );
}

// ---- per-view hint sets ----------------------------------------------------

const JOBS_HINTS: [Hint; 19] = [
    Hint {
        key: "1·2·3",
        label: "view",
        kind: HintKind::Normal,
    },
    Hint {
        key: "↑↓",
        label: "select",
        kind: HintKind::Normal,
    },
    Hint {
        key: "PgUp·PgDn",
        label: "page",
        kind: HintKind::Normal,
    },
    Hint {
        key: "Tab",
        label: "group",
        kind: HintKind::Normal,
    },
    Hint {
        key: "Enter",
        label: "open",
        kind: HintKind::Normal,
    },
    Hint {
        key: "l",
        label: "logs",
        kind: HintKind::Normal,
    },
    Hint {
        key: "c",
        label: "cancel",
        kind: HintKind::Danger,
    },
    Hint {
        key: "h",
        label: "hold",
        kind: HintKind::Danger,
    },
    Hint {
        key: "Q",
        label: "requeue",
        kind: HintKind::Danger,
    },
    Hint {
        key: "/",
        label: "filter",
        kind: HintKind::Normal,
    },
    Hint {
        key: "a",
        label: "me/all",
        kind: HintKind::Normal,
    },
    Hint {
        key: "s",
        label: "sort",
        kind: HintKind::Normal,
    },
    Hint {
        key: "R",
        label: "refresh",
        kind: HintKind::Normal,
    },
    Hint {
        key: "Ctrl+K",
        label: "assist",
        kind: HintKind::Special,
    },
    Hint {
        key: "w",
        label: "web UI",
        kind: HintKind::Special,
    },
    Hint {
        key: ",",
        label: "settings",
        kind: HintKind::Normal,
    },
    Hint {
        key: "T",
        label: "theme",
        kind: HintKind::Normal,
    },
    Hint {
        key: "?",
        label: "help",
        kind: HintKind::Normal,
    },
    Hint {
        key: "q",
        label: "quit",
        kind: HintKind::Normal,
    },
];

const SETTINGS_HINTS: [Hint; 7] = [
    Hint {
        key: "↑↓",
        label: "select",
        kind: HintKind::Normal,
    },
    Hint {
        key: "e·Enter",
        label: "edit",
        kind: HintKind::Special,
    },
    Hint {
        key: "t",
        label: "test LLM",
        kind: HintKind::Special,
    },
    Hint {
        key: "w",
        label: "start web UI",
        kind: HintKind::Special,
    },
    Hint {
        key: "T",
        label: "theme",
        kind: HintKind::Normal,
    },
    Hint {
        key: "Esc",
        label: "back",
        kind: HintKind::Normal,
    },
    Hint {
        key: "q",
        label: "quit",
        kind: HintKind::Normal,
    },
];

const DETAILS_HINTS: [Hint; 2] = [
    Hint {
        key: "Esc",
        label: "back",
        kind: HintKind::Normal,
    },
    Hint {
        key: "q",
        label: "quit",
        kind: HintKind::Normal,
    },
];

const LOGS_HINTS: [Hint; 9] = [
    Hint {
        key: "↑↓",
        label: "scroll",
        kind: HintKind::Normal,
    },
    Hint {
        key: "PgUp·PgDn",
        label: "page",
        kind: HintKind::Normal,
    },
    Hint {
        key: "g·G",
        label: "top/bot",
        kind: HintKind::Normal,
    },
    Hint {
        key: "f",
        label: "follow",
        kind: HintKind::Normal,
    },
    Hint {
        key: "/",
        label: "search",
        kind: HintKind::Normal,
    },
    Hint {
        key: "n",
        label: "next",
        kind: HintKind::Normal,
    },
    Hint {
        key: "R",
        label: "refresh",
        kind: HintKind::Normal,
    },
    Hint {
        key: "Esc",
        label: "back",
        kind: HintKind::Normal,
    },
    Hint {
        key: "q",
        label: "quit",
        kind: HintKind::Normal,
    },
];
