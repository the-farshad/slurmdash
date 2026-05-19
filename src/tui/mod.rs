//! Terminal UI: setup, teardown, event loop.

pub mod format;
pub mod theme;
pub mod widgets;

use std::io;
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::event::{
    DisableMouseCapture, EnableMouseCapture, Event, EventStream, KeyCode, KeyEventKind,
    KeyModifiers,
};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use futures::StreamExt;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};

use crate::actions::ActionKind;
use crate::app::{AppState, Confirm, View};
use crate::cli::Cli;
use crate::config::Config;
use crate::db::{Db, snapshots};
use crate::slurm::{scontrol, squeue};
use crate::ssh::{Runner, RunnerHandle};
use crate::tui::theme::Theme;

type Tui = Terminal<CrosstermBackend<io::Stdout>>;

pub async fn run(
    cli: Cli,
    config: Config,
    handle: RunnerHandle,
    db: Option<Db>,
) -> Result<()> {
    let mut terminal = setup_terminal()?;
    let result = run_loop(&mut terminal, &cli, &config, &handle, db.as_ref()).await;
    restore_terminal(&mut terminal)?;
    result
}

fn setup_terminal() -> Result<Tui> {
    enable_raw_mode().context("enabling raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
        .context("entering alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend).context("creating terminal")
}

fn restore_terminal(terminal: &mut Tui) -> Result<()> {
    disable_raw_mode().ok();
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )
    .ok();
    terminal.show_cursor().ok();
    Ok(())
}

async fn run_loop(
    terminal: &mut Tui,
    cli: &Cli,
    config: &Config,
    handle: &RunnerHandle,
    db: Option<&Db>,
) -> Result<()> {
    let theme = Theme::from_name(
        cli.theme
            .as_deref()
            .unwrap_or(&config.ui.theme),
    );
    let refresh_secs = cli.refresh.unwrap_or(config.ui.refresh_seconds).max(1);
    let cluster_label = handle.cluster_name.clone();
    let runner: &dyn Runner = handle.runner.as_ref();

    // Resolve cluster_id once for the session.
    let cluster_id = match db {
        Some(d) => snapshots::ensure_cluster(&d.pool, &handle.cluster_name, handle.is_local)
            .await
            .ok(),
        None => None,
    };

    let opts = squeue::Options {
        me: !cli.all,
        user: None,
        partition: cli.partition.clone(),
        state: cli.state.clone(),
        extra_args: Vec::new(),
    };

    let mut state = AppState::default();
    let mut last_refresh = None;

    fetch(runner, &opts, &mut state, &mut last_refresh, db, cluster_id).await;

    let mut events = EventStream::new();
    let mut ticker = tokio::time::interval(Duration::from_secs(refresh_secs));
    ticker.tick().await;

    loop {
        terminal.draw(|frame| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1),
                    Constraint::Min(1),
                    Constraint::Length(1),
                ])
                .split(frame.area());

            widgets::header::render(
                frame,
                chunks[0],
                &state,
                &theme,
                &cluster_label,
                last_refresh,
                false,
            );

            match state.view {
                View::Jobs => {
                    widgets::job_table::render(frame, chunks[1], &state, &theme);
                }
                View::Details => {
                    widgets::details::render(frame, chunks[1], &state, &theme);
                }
            }

            widgets::footer::render(frame, chunks[2], &theme, state.view);

            if state.show_help {
                widgets::help::render(frame, frame.area(), &theme);
            }
            if let Some(confirm) = &state.confirm {
                widgets::confirm::render(frame, frame.area(), confirm, &theme);
            }
            if let Some(err) = &state.last_error {
                widgets::error_banner::render(frame, frame.area(), err, &theme);
            }
        })?;

        tokio::select! {
            Some(Ok(event)) = events.next() => {
                let intent = handle_event(event, &mut state);
                match intent {
                    Intent::None => {}
                    Intent::Quit => break,
                    Intent::Refresh => {
                        fetch(runner, &opts, &mut state, &mut last_refresh, db, cluster_id).await;
                    }
                    Intent::OpenDetails => {
                        if let Some(job) = state.selected_job() {
                            let job_id = job.job_id.clone();
                            match scontrol::show(runner, &job_id).await {
                                Ok(d) => {
                                    state.details = Some(d);
                                    state.view = View::Details;
                                }
                                Err(e) => state.last_error = Some(format!("{e}")),
                            }
                        }
                    }
                    Intent::ConfirmAction => {
                        if let Some(c) = state.confirm.take() {
                            let result = crate::actions::run(
                                c.kind,
                                &c.job_id,
                                runner,
                                db,
                                &handle.cluster_name,
                                handle.is_local,
                                true,
                            )
                            .await;
                            match result {
                                Ok(()) => {
                                    fetch(runner, &opts, &mut state, &mut last_refresh, db, cluster_id).await;
                                }
                                Err(e) => state.last_error = Some(format!("{e}")),
                            }
                        }
                    }
                }
                if state.should_quit { break; }
            }
            _ = ticker.tick() => {
                fetch(runner, &opts, &mut state, &mut last_refresh, db, cluster_id).await;
            }
        }
    }
    Ok(())
}

enum Intent {
    None,
    Quit,
    Refresh,
    OpenDetails,
    ConfirmAction,
}

fn handle_event(event: Event, state: &mut AppState) -> Intent {
    let Event::Key(key) = event else { return Intent::None };
    if key.kind != KeyEventKind::Press {
        return Intent::None;
    }

    // Ctrl-C always exits.
    if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
        state.should_quit = true;
        return Intent::Quit;
    }

    // Modal: confirm dialog absorbs keys.
    if state.confirm.is_some() {
        match key.code {
            KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => return Intent::ConfirmAction,
            KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                state.confirm = None;
            }
            _ => {}
        }
        return Intent::None;
    }

    // Modal: help overlay closes on any key.
    if state.show_help {
        state.show_help = false;
        return Intent::None;
    }

    // Modal: error banner dismisses on any key.
    if state.last_error.is_some() {
        state.last_error = None;
        // fall through so the same key can also act on the underlying view
    }

    if state.view == View::Details {
        if matches!(
            key.code,
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Backspace
        ) {
            state.view = View::Jobs;
            state.details = None;
            return Intent::None;
        }
    }

    match key.code {
        KeyCode::Char('q') => {
            state.should_quit = true;
            Intent::Quit
        }
        KeyCode::Char('j') | KeyCode::Down => {
            state.select_next();
            Intent::None
        }
        KeyCode::Char('k') | KeyCode::Up => {
            state.select_prev();
            Intent::None
        }
        KeyCode::Char('g') | KeyCode::Home => {
            state.selected = 0;
            Intent::None
        }
        KeyCode::Char('G') | KeyCode::End => {
            state.selected = state.jobs.len().saturating_sub(1);
            Intent::None
        }
        KeyCode::Char('R') | KeyCode::Char('r') => Intent::Refresh,
        KeyCode::Enter | KeyCode::Char('d') => Intent::OpenDetails,
        KeyCode::Char('?') => {
            state.show_help = true;
            Intent::None
        }
        KeyCode::Char('c') => open_confirm(state, ActionKind::Cancel),
        KeyCode::Char('h') => open_confirm(state, ActionKind::Hold),
        KeyCode::Char('u') => open_confirm(state, ActionKind::Release),
        // 'r' is overloaded for refresh; use 'Q' for requeue to avoid clobbering.
        KeyCode::Char('Q') => open_confirm(state, ActionKind::Requeue),
        _ => Intent::None,
    }
}

fn open_confirm(state: &mut AppState, kind: ActionKind) -> Intent {
    if let Some(job) = state.selected_job() {
        state.confirm = Some(Confirm {
            kind,
            job_id: job.job_id.clone(),
            preview: kind.preview(&job.job_id),
        });
    }
    Intent::None
}

async fn fetch(
    runner: &dyn Runner,
    opts: &squeue::Options,
    state: &mut AppState,
    last_refresh: &mut Option<chrono::DateTime<chrono::Utc>>,
    db: Option<&Db>,
    cluster_id: Option<i64>,
) {
    match squeue::list(runner, opts).await {
        Ok(jobs) => {
            if state.selected >= jobs.len() && !jobs.is_empty() {
                state.selected = jobs.len() - 1;
            }
            if let (Some(d), Some(cid)) = (db, cluster_id) {
                let _ = snapshots::write_jobs(&d.pool, cid, &jobs).await;
            }
            state.jobs = jobs;
            state.last_error = None;
            *last_refresh = Some(chrono::Utc::now());
        }
        Err(e) => {
            state.last_error = Some(format!("{e}"));
        }
    }
}
