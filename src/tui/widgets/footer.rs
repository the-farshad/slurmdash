//! Keybinding footer with colored hints.
//!
//! Each hint is a `[key] label` pair. The key gets the accent color (or
//! red for destructive actions, magenta for special chord keys); the label
//! stays muted. Errors take precedence over the hint line and dismiss on
//! the next successful refresh.

use ratatui::Frame;
use ratatui::layout::Rect;
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
        frame.render_widget(Paragraph::new(line), area);
        return;
    }

    let hints: &[Hint] = match state.view {
        View::Dashboard | View::Jobs | View::Statistics => &JOBS_HINTS,
        View::Details => &DETAILS_HINTS,
        View::Logs => &LOGS_HINTS,
        View::Settings => &SETTINGS_HINTS,
    };

    let mut spans = Vec::with_capacity(hints.len() * 4 + 1);
    for (i, h) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ", Style::default().fg(theme.border)));
        }
        let key_color = match h.kind {
            HintKind::Normal => theme.accent,
            HintKind::Danger => theme.action_danger,
            HintKind::Special => theme.cancelled, // purple-ish, distinct from accent + danger
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
        area,
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

const SETTINGS_HINTS: [Hint; 5] = [
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
