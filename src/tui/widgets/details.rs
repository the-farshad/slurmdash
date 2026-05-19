use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::app::AppState;
use crate::slurm::state::JobState;
use crate::tui::theme::Theme;

pub fn render(frame: &mut Frame<'_>, area: Rect, state: &AppState, theme: &Theme) {
    let Some(d) = &state.details else {
        let p = Paragraph::new("(no details loaded)");
        frame.render_widget(p, area);
        return;
    };

    // Reserve a single line for a progress bar when the selected job is
    // running and has both elapsed and time-limit known. Otherwise the
    // details paragraph uses the full area.
    let progress_visible = state
        .selected_job()
        .map(|j| {
            j.state == JobState::Running
                && j.elapsed_seconds.is_some()
                && j.time_limit_seconds.is_some()
        })
        .unwrap_or(false);

    let (progress_area, body_area) = if progress_visible {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(1)])
            .split(area);
        (Some(chunks[0]), chunks[1])
    } else {
        (None, area)
    };

    if let (Some(pa), Some(job)) = (progress_area, state.selected_job()) {
        super::progress::render(
            frame,
            pa,
            "Time",
            job.elapsed_seconds.unwrap_or(0),
            job.time_limit_seconds.unwrap_or(0),
            theme,
        );
    }

    let label_style = theme.header_style();
    let muted_style = theme.footer_style();

    let mut lines = Vec::new();
    macro_rules! kv {
        ($key:expr, $val:expr) => {
            if let Some(v) = $val {
                lines.push(Line::from(vec![
                    Span::styled(format!("  {:<12}", $key), label_style),
                    Span::raw(v.to_string()),
                ]));
            }
        };
    }

    lines.push(Line::from(Span::styled(
        format!(" Job {}", d.job_id),
        theme.header_style(),
    )));
    lines.push(Line::raw(""));

    // Pending-reason explainer: render before the key/value block so users
    // see the action item immediately.
    if let (Some(state_name), Some(reason)) = (&d.state, &d.reason) {
        if state_name.eq_ignore_ascii_case("PENDING") {
            let explained = crate::slurm::reason::explain(reason);
            lines.push(Line::from(Span::styled(
                format!("  Reason  {} — {}", explained.code, explained.summary),
                theme.header_style(),
            )));
            if let Some(suggestion) = explained.suggestion {
                lines.push(Line::from(Span::styled(
                    format!("          {suggestion}"),
                    theme.footer_style(),
                )));
            }
            lines.push(Line::raw(""));
        }
    }
    kv!("Name", d.job_name.as_deref());
    kv!("User", d.user.as_deref());
    kv!("Account", d.account.as_deref());
    kv!("Partition", d.partition.as_deref());
    kv!("QoS", d.qos.as_deref());
    kv!("State", d.state.as_deref());
    kv!("Reason", d.reason.as_deref());
    kv!("Priority", d.priority.as_deref());
    kv!("Dependency", d.dependency.as_deref());
    kv!("Command", d.command.as_deref());
    kv!("WorkDir", d.workdir.as_deref());
    kv!("StdOut", d.stdout.as_deref());
    kv!("StdErr", d.stderr.as_deref());
    kv!("NodeList", d.nodes_alloc.as_deref());
    kv!("ExitCode", d.exit_code.as_deref());

    if !d.raw.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::from(Span::styled(" raw fields", muted_style)));
        for (k, v) in &d.raw {
            lines.push(Line::from(vec![
                Span::styled(format!("  {k}="), muted_style),
                Span::raw(v.clone()),
            ]));
        }
    }

    let block = Block::default()
        .borders(Borders::TOP | Borders::BOTTOM)
        .border_style(theme.border_style());
    frame.render_widget(
        Paragraph::new(lines).block(block).wrap(Wrap { trim: false }),
        body_area,
    );
}
