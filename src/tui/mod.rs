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

use crate::app::AppState;
use crate::cli::Cli;
use crate::config::Config;
use crate::db::Db;
use crate::slurm::squeue;
use crate::ssh::Runner;
use crate::tui::theme::Theme;

type Tui = Terminal<CrosstermBackend<io::Stdout>>;

pub async fn run(
    cli: Cli,
    config: Config,
    runner: Box<dyn Runner>,
    _db: Option<Db>,
) -> Result<()> {
    let mut terminal = setup_terminal()?;
    let result = run_loop(&mut terminal, &cli, &config, runner.as_ref()).await;
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
    runner: &dyn Runner,
) -> Result<()> {
    let theme = Theme::from_name(
        cli.theme
            .as_deref()
            .unwrap_or(&config.ui.theme),
    );
    let refresh_secs = cli.refresh.unwrap_or(config.ui.refresh_seconds).max(1);
    let cluster_label = runner.description();

    let opts = squeue::Options {
        me: !cli.all,
        user: None,
        partition: cli.partition.clone(),
        state: cli.state.clone(),
        extra_args: Vec::new(),
    };

    let mut state = AppState::default();
    let mut last_refresh = None;

    // Initial fetch — blocking so we have data before the first paint.
    fetch(runner, &opts, &mut state, &mut last_refresh).await;

    let mut events = EventStream::new();
    let mut ticker = tokio::time::interval(Duration::from_secs(refresh_secs));
    ticker.tick().await; // consume the immediate tick

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
            widgets::job_table::render(frame, chunks[1], &state, &theme);
            widgets::footer::render(frame, chunks[2], &theme);
        })?;

        tokio::select! {
            Some(Ok(event)) = events.next() => {
                if handle_event(event, &mut state) {
                    // user requested a manual refresh
                    fetch(runner, &opts, &mut state, &mut last_refresh).await;
                }
                if state.should_quit {
                    break;
                }
            }
            _ = ticker.tick() => {
                fetch(runner, &opts, &mut state, &mut last_refresh).await;
            }
        }
    }
    Ok(())
}

/// Returns true if the event signals a manual refresh.
fn handle_event(event: Event, state: &mut AppState) -> bool {
    match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => {
            if key.modifiers.contains(KeyModifiers::CONTROL)
                && matches!(key.code, KeyCode::Char('c'))
            {
                state.should_quit = true;
                return false;
            }
            match key.code {
                KeyCode::Char('q') => state.should_quit = true,
                KeyCode::Char('j') | KeyCode::Down => state.select_next(),
                KeyCode::Char('k') | KeyCode::Up => state.select_prev(),
                KeyCode::Char('g') | KeyCode::Home => state.selected = 0,
                KeyCode::Char('G') | KeyCode::End => {
                    state.selected = state.jobs.len().saturating_sub(1);
                }
                KeyCode::Char('R') | KeyCode::Char('r') => return true,
                _ => {}
            }
        }
        Event::Mouse(_) => {}
        Event::Resize(_, _) => {}
        _ => {}
    }
    false
}

async fn fetch(
    runner: &dyn Runner,
    opts: &squeue::Options,
    state: &mut AppState,
    last_refresh: &mut Option<chrono::DateTime<chrono::Utc>>,
) {
    match squeue::list(runner, opts).await {
        Ok(jobs) => {
            state.jobs = jobs;
            state.last_error = None;
            if state.selected >= state.jobs.len() && !state.jobs.is_empty() {
                state.selected = state.jobs.len() - 1;
            }
            *last_refresh = Some(chrono::Utc::now());
        }
        Err(e) => {
            state.last_error = Some(format!("{e}"));
        }
    }
}
