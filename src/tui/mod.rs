//! Terminal UI: setup, teardown, event loop.

pub mod format;
pub mod theme;
pub mod widgets;

use std::io;
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::event::{
    DisableMouseCapture, EnableMouseCapture, Event, EventStream, KeyCode, KeyEventKind,
    KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use futures::StreamExt;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};

use crate::actions::ActionKind;
use crate::app::{
    AppState, Confirm, LogKind, LogView, ResourceSample, View, apply_sort,
};
use crate::cli::Cli;
use crate::config::Config;
use crate::db::{Db, snapshots};
use crate::slurm::model::ClusterResources;
use crate::slurm::{scontrol, sinfo, squeue};
use crate::ssh::{LineStream, Runner, RunnerHandle};
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
    let theme = Theme::from_name(cli.theme.as_deref().unwrap_or(&config.ui.theme));
    let refresh_secs = cli.refresh.unwrap_or(config.ui.refresh_seconds).max(1);
    let cluster_label = handle.cluster_name.clone();
    let runner: &dyn Runner = handle.runner.as_ref();

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
    let mut log_stream: Option<LineStream> = None;

    fetch(runner, &opts, &mut state, &mut last_refresh, db, cluster_id).await;
    fetch_sinfo(runner, &mut state, db, cluster_id).await;

    let mut events = EventStream::new();
    let mut ticker = tokio::time::interval(Duration::from_secs(refresh_secs));
    ticker.tick().await;

    loop {
        let table_rect = draw(terminal, &state, &theme, &cluster_label, last_refresh)?;
        state.table_rect = table_rect;

        tokio::select! {
            biased;
            log_event = recv_log_line(log_stream.as_mut()) => {
                match log_event {
                    Some(line) => if let Some(log) = state.log.as_mut() {
                        log.push(line);
                    },
                    None => log_stream = None,
                }
            }
            Some(Ok(event)) = events.next() => {
                let intent = handle_event(event, &mut state);
                match intent {
                    Intent::None => {}
                    Intent::Quit => break,
                    Intent::Refresh => {
                        fetch(runner, &opts, &mut state, &mut last_refresh, db, cluster_id).await;
                        fetch_sinfo(runner, &mut state, db, cluster_id).await;
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
                    Intent::OpenLog(kind) => {
                        if let Some(job) = state.selected_job() {
                            let job_id = job.job_id.clone();
                            match open_log(runner, &job_id, kind).await {
                                Ok((view, stream)) => {
                                    state.log = Some(view);
                                    state.view = View::Logs;
                                    log_stream = Some(stream);
                                }
                                Err(e) => state.last_error = Some(format!("{e}")),
                            }
                        }
                    }
                    Intent::CloseLog => {
                        state.view = View::Dashboard;
                        state.log = None;
                        log_stream = None;
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
                if state.view != View::Logs {
                    fetch(runner, &opts, &mut state, &mut last_refresh, db, cluster_id).await;
                    fetch_sinfo(runner, &mut state, db, cluster_id).await;
                }
            }
        }
    }
    Ok(())
}

/// Returns the area where the job table was rendered (used to translate
/// mouse clicks into row indices on the next event). `None` if the current
/// view doesn't show the table.
fn draw(
    terminal: &mut Tui,
    state: &AppState,
    theme: &Theme,
    cluster_label: &str,
    last_refresh: Option<chrono::DateTime<chrono::Utc>>,
) -> Result<Option<Rect>> {
    let mut table_rect: Option<Rect> = None;
    terminal.draw(|frame| {
        let outer = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(1),
                Constraint::Length(1),
            ])
            .split(frame.area());

        widgets::header::render(
            frame,
            outer[0],
            state,
            theme,
            cluster_label,
            last_refresh,
            false,
        );

        match state.view {
            View::Dashboard => {
                table_rect = render_dashboard(frame, outer[1], state, theme);
            }
            View::Jobs => {
                widgets::job_table::render(frame, outer[1], state, theme);
                table_rect = Some(outer[1]);
            }
            View::Details => {
                widgets::details::render(frame, outer[1], state, theme);
            }
            View::Logs => {
                if let Some(log) = state.log.as_ref() {
                    let log_layout = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Length(1), Constraint::Min(1)])
                        .split(outer[1]);
                    widgets::log_viewer::render_header(
                        frame,
                        log_layout[0],
                        log,
                        theme,
                        state.search_input.as_deref(),
                        state.sort,
                    );
                    widgets::log_viewer::render_body(frame, log_layout[1], log, theme);
                }
            }
        }

        widgets::footer::render(frame, outer[2], theme, state.view);

        if state.show_help {
            widgets::help::render(frame, frame.area(), theme);
        }
        if let Some(confirm) = &state.confirm {
            widgets::confirm::render(frame, frame.area(), confirm, theme);
        }
        if let Some(err) = &state.last_error {
            widgets::error_banner::render(frame, frame.area(), err, theme);
        }
    })?;
    Ok(table_rect)
}

/// Three-stack dashboard: top row (resources + queue + ending-soon),
/// middle row (partition cards), bottom row (job table). Returns the rect
/// of the job table so mouse clicks can still drive selection.
fn render_dashboard(
    frame: &mut ratatui::Frame<'_>,
    area: Rect,
    state: &AppState,
    theme: &Theme,
) -> Option<Rect> {
    let part_rows = (state.partitions.len() as u16 + 2).clamp(3, 12);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(9),
            Constraint::Length(part_rows),
            Constraint::Min(5),
        ])
        .split(area);

    widgets::sparkline::render(frame, chunks[0], &state.resource_history, theme);

    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(40),
            Constraint::Percentage(30),
            Constraint::Percentage(30),
        ])
        .split(chunks[1]);

    widgets::resources::render(frame, top[0], &state.resources, theme);
    widgets::queue::render(frame, top[1], &state.jobs, theme);
    widgets::ending_soon::render(frame, top[2], &state.jobs, theme);

    widgets::partitions::render(frame, chunks[2], &state.partitions, theme);
    widgets::job_table::render(frame, chunks[3], state, theme);

    Some(chunks[3])
}

async fn recv_log_line(stream: Option<&mut LineStream>) -> Option<String> {
    match stream {
        Some(s) => s.rx.recv().await,
        None => std::future::pending().await,
    }
}

enum Intent {
    None,
    Quit,
    Refresh,
    OpenDetails,
    OpenLog(LogKind),
    CloseLog,
    ConfirmAction,
}

fn handle_event(event: Event, state: &mut AppState) -> Intent {
    match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => handle_key(key, state),
        Event::Mouse(m) => {
            handle_mouse(m, state);
            Intent::None
        }
        _ => Intent::None,
    }
}

fn handle_key(key: crossterm::event::KeyEvent, state: &mut AppState) -> Intent {
    if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
        state.should_quit = true;
        return Intent::Quit;
    }

    if let Some(buf) = state.search_input.as_mut() {
        match key.code {
            KeyCode::Esc => {
                state.search_input = None;
            }
            KeyCode::Enter => {
                let q = state.search_input.take().unwrap_or_default();
                if let Some(log) = state.log.as_mut() {
                    log.search = if q.is_empty() { None } else { Some(q) };
                    log.find_next(log.scroll);
                }
            }
            KeyCode::Backspace => {
                buf.pop();
            }
            KeyCode::Char(c) => {
                buf.push(c);
            }
            _ => {}
        }
        return Intent::None;
    }

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

    if state.show_help {
        state.show_help = false;
        return Intent::None;
    }

    if state.last_error.is_some() {
        state.last_error = None;
    }

    // View switches are global (skip in input modes).
    match key.code {
        KeyCode::Char('1') => {
            state.view = View::Dashboard;
            return Intent::None;
        }
        KeyCode::Char('2') => {
            state.view = View::Jobs;
            return Intent::None;
        }
        _ => {}
    }

    match state.view {
        View::Details => handle_key_details(key, state),
        View::Logs => handle_key_logs(key, state),
        View::Dashboard | View::Jobs => handle_key_jobs(key, state),
    }
}

fn handle_key_jobs(key: crossterm::event::KeyEvent, state: &mut AppState) -> Intent {
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
        KeyCode::Char('l') => Intent::OpenLog(LogKind::Stdout),
        KeyCode::Char('e') => Intent::OpenLog(LogKind::Stderr),
        KeyCode::Char('s') => {
            state.sort.key = state.sort.key.next();
            apply_sort(&mut state.jobs, state.sort);
            Intent::None
        }
        KeyCode::Char('S') => {
            state.sort.reverse = !state.sort.reverse;
            apply_sort(&mut state.jobs, state.sort);
            Intent::None
        }
        KeyCode::Char('?') => {
            state.show_help = true;
            Intent::None
        }
        KeyCode::Char('c') => open_confirm(state, ActionKind::Cancel),
        KeyCode::Char('h') => open_confirm(state, ActionKind::Hold),
        KeyCode::Char('u') => open_confirm(state, ActionKind::Release),
        KeyCode::Char('Q') => open_confirm(state, ActionKind::Requeue),
        _ => Intent::None,
    }
}

fn handle_key_details(key: crossterm::event::KeyEvent, state: &mut AppState) -> Intent {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Backspace => {
            state.view = View::Dashboard;
            state.details = None;
        }
        KeyCode::Char('?') => state.show_help = true,
        _ => {}
    }
    Intent::None
}

fn handle_key_logs(key: crossterm::event::KeyEvent, state: &mut AppState) -> Intent {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Backspace => return Intent::CloseLog,
        KeyCode::Char('?') => state.show_help = true,
        KeyCode::Char('f') => {
            if let Some(log) = state.log.as_mut() {
                log.follow = !log.follow;
            }
        }
        KeyCode::Char('g') | KeyCode::Home => {
            if let Some(log) = state.log.as_mut() {
                log.follow = false;
                log.scroll = 0;
            }
        }
        KeyCode::Char('G') | KeyCode::End => {
            if let Some(log) = state.log.as_mut() {
                log.follow = true;
            }
        }
        KeyCode::Char('j') | KeyCode::Down => {
            if let Some(log) = state.log.as_mut() {
                log.scroll_by(1);
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if let Some(log) = state.log.as_mut() {
                log.scroll_by(-1);
            }
        }
        KeyCode::PageDown => {
            if let Some(log) = state.log.as_mut() {
                log.scroll_by(20);
            }
        }
        KeyCode::PageUp => {
            if let Some(log) = state.log.as_mut() {
                log.scroll_by(-20);
            }
        }
        KeyCode::Char('/') => {
            state.search_input = Some(String::new());
        }
        KeyCode::Char('n') => {
            if let Some(log) = state.log.as_mut() {
                let from = log.scroll.saturating_add(1);
                log.find_next(from);
            }
        }
        _ => {}
    }
    Intent::None
}

fn handle_mouse(m: MouseEvent, state: &mut AppState) {
    let MouseEvent { kind, column, row, .. } = m;
    let Some(table_rect) = state.table_rect else { return };
    if !matches!(state.view, View::Dashboard | View::Jobs) {
        return;
    }

    let inside = column >= table_rect.x
        && column < table_rect.x + table_rect.width
        && row >= table_rect.y
        && row < table_rect.y + table_rect.height;

    if !inside {
        return;
    }

    match kind {
        MouseEventKind::Down(MouseButton::Left) => {
            let header_offset: u16 = 1;
            let rel = row.saturating_sub(table_rect.y + header_offset);
            let idx = rel as usize;
            if idx < state.jobs.len() {
                state.selected = idx;
            }
        }
        MouseEventKind::ScrollDown => state.select_next(),
        MouseEventKind::ScrollUp => state.select_prev(),
        _ => {}
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

async fn open_log(
    runner: &dyn Runner,
    job_id: &str,
    kind: LogKind,
) -> Result<(LogView, LineStream)> {
    let details = scontrol::show(runner, job_id).await?;
    let path = match kind {
        LogKind::Stdout => details.stdout.clone(),
        LogKind::Stderr => details.stderr.clone(),
    }
    .filter(|p| !p.is_empty())
    .ok_or_else(|| anyhow::anyhow!("no {} path in scontrol output", kind.label()))?;

    let stream = runner
        .stream("tail", &["-F", "-n", "200", path.as_str()])
        .await?;
    Ok((LogView::new(job_id.to_string(), kind, path), stream))
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
        Ok(mut jobs) => {
            apply_sort(&mut jobs, state.sort);
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

async fn fetch_sinfo(
    runner: &dyn Runner,
    state: &mut AppState,
    db: Option<&Db>,
    cluster_id: Option<i64>,
) {
    match sinfo::list_partitions(runner).await {
        Ok(parts) => {
            let resources = ClusterResources::from_partitions(&parts);
            state.push_resource_sample(ResourceSample::from(chrono::Utc::now(), &resources));
            state.resources = resources;
            if let (Some(d), Some(cid)) = (db, cluster_id) {
                let _ = snapshots::write_resources(&d.pool, cid, &parts).await;
            }
            state.partitions = parts;
        }
        Err(e) => {
            // Non-fatal: keep the previous snapshot, surface in the banner.
            tracing::warn!(error = %e, "sinfo refresh failed");
        }
    }
}
