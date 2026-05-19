//! Color-graded progress bar used for "elapsed vs. time-limit" and similar
//! ratios. Renders as `[████████░░░░] 67%  01:24:00 / 02:00:00`.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::tui::format::hms;
use crate::tui::theme::Theme;

/// Render a single-line progress bar inside `area`.
///
/// `done` and `total` are the numerator and denominator; the bar uses the
/// usage-gradient palette (green → yellow → orange → red) for the bar fill.
pub fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    label: &str,
    done: u64,
    total: u64,
    theme: &Theme,
) {
    let width = area.width.saturating_sub(label.len() as u16 + 20);
    let pct = if total == 0 {
        0.0
    } else {
        (done as f64 / total as f64).clamp(0.0, 1.0)
    };
    let bar_color = gradient_color(pct, theme);
    // Top-of-Details "Time" bar uses the same Braille dots as the
    // History block below it for visual continuity — the whole
    // Details view is the dot aesthetic.
    let (bar_fill, bar_rest) = super::braille::bar_pair(pct, width as usize);

    let line = Line::from(vec![
        Span::styled(format!("{label:<6}"), theme.footer_style()),
        Span::raw("["),
        Span::styled(bar_fill, Style::default().fg(bar_color)),
        Span::styled(bar_rest, theme.footer_style()),
        Span::raw("]"),
        Span::raw(format!(" {:>3}%", (pct * 100.0) as u32)),
        Span::raw(" "),
        Span::styled(
            format!("{} / {}", hms(done), hms(total)),
            theme.footer_style(),
        ),
    ]);

    frame.render_widget(Paragraph::new(line), area);
}

fn gradient_color(pct: f64, theme: &Theme) -> Color {
    match (pct * 100.0) as u32 {
        0..=49 => theme.usage_low,
        50..=79 => theme.usage_med,
        80..=94 => theme.usage_high,
        _ => theme.usage_critical,
    }
}
