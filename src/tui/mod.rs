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
    AppState, AssistDialog, Confirm, FilterMode, LogKind, LogView, ResourceSample, View, apply_sort,
};
use crate::assist::{AssistRequest, JobContext, ProposedKind};
use crate::cli::Cli;
use crate::config::Config;
use crate::db::{Db, snapshots};
use crate::slurm::model::ClusterResources;
use crate::slurm::{scontrol, sinfo, squeue};
use crate::ssh::{LineStream, Runner, RunnerHandle};
use crate::tui::theme::Theme;

type Tui = Terminal<CrosstermBackend<io::Stdout>>;

pub async fn run(cli: Cli, config: Config, handle: RunnerHandle, db: Option<Db>) -> Result<()> {
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

/// Result of a background refresh task, sent back through `LoopMsg`.
enum LoopMsg {
    Jobs(Result<Vec<crate::slurm::model::Job>>),
    Partitions(Result<Vec<crate::slurm::model::Partition>>),
    /// LLM probe-test response from the Settings `t` key.
    AssistTest(Result<crate::assist::AssistResponse>),
    /// Response for the Ctrl+K assist dialog.
    AssistComplete(Result<crate::assist::AssistResponse>),
}

async fn run_loop(
    terminal: &mut Tui,
    cli: &Cli,
    config: &Config,
    handle: &RunnerHandle,
    db: Option<&Db>,
) -> Result<()> {
    let refresh_secs = cli.refresh.unwrap_or(config.ui.refresh_seconds).max(1);
    let cluster_label = handle.cluster_name.clone();

    let cluster_id = match db {
        Some(d) => snapshots::ensure_cluster(&d.pool, &handle.cluster_name, handle.is_local)
            .await
            .ok(),
        None => None,
    };

    // Resolve the initial theme. Priority: CLI flag > saved settings > config > "dark".
    let saved_theme = match db {
        Some(d) => crate::db::settings::get_theme(&d.pool).await.ok().flatten(),
        None => None,
    };
    let initial_theme = cli
        .theme
        .clone()
        .or(saved_theme)
        .unwrap_or_else(|| config.ui.theme.clone());

    // Resolve LLM config. Hierarchy: existing env vars > DB-stored
    // assist settings > built-in defaults. Whatever wins is written
    // back to env vars so the assist providers (which read env at call
    // time) see the same values shown in the Settings view.
    let stored_llm = match db {
        Some(d) => crate::db::settings::get_assist(&d.pool)
            .await
            .ok()
            .flatten()
            .unwrap_or_default(),
        None => crate::db::settings::AssistSettings::default(),
    };
    let llm = crate::app::LlmConfig {
        provider: std::env::var("SLURMDASH_LLM_PROVIDER")
            .ok()
            .or(stored_llm.provider)
            .unwrap_or_else(|| "ollama".into()),
        ollama_host: std::env::var("OLLAMA_HOST")
            .ok()
            .or(stored_llm.ollama_host)
            .unwrap_or_else(|| "http://localhost:11434".into()),
        ollama_model: std::env::var("OLLAMA_MODEL")
            .ok()
            .or(stored_llm.ollama_model)
            .unwrap_or_else(|| "llama3.2".into()),
        anthropic_model: std::env::var("ANTHROPIC_MODEL")
            .ok()
            .or(stored_llm.anthropic_model)
            .unwrap_or_else(|| "claude-sonnet-4-6".into()),
    };
    apply_llm_to_env(&llm);

    let mut state = AppState {
        filter: if cli.all {
            FilterMode::All
        } else {
            FilterMode::Me
        },
        theme_name: initial_theme,
        ..AppState::default()
    };
    state.settings.llm = llm;
    let mut last_refresh = None;
    let mut log_stream: Option<LineStream> = None;

    // Bounded mpsc channel for refresh results. Small capacity is fine —
    // results are processed immediately by the select loop.
    let (tx, mut rx) = tokio::sync::mpsc::channel::<LoopMsg>(8);

    // `runner` is reused below for actions that must be synchronous (scontrol
    // show, log tail open, action dispatch). Refreshes use the same Arc
    // cloned into spawned tasks via `trigger_refresh`.
    let runner: &dyn Runner = handle.runner.as_ref();

    // Kick off the first refresh in the background and let the loop paint
    // immediately. The "Loading…" placeholders in empty panels signal that
    // data is on its way without blocking startup.
    trigger_refresh(handle, cli, db, cluster_id, &mut state, &tx);

    let mut events = EventStream::new();
    let mut ticker = tokio::time::interval(Duration::from_secs(refresh_secs));
    ticker.tick().await;
    // 200ms redraw ticker so the loading spinner animates while a fetch
    // is in flight (and is otherwise a cheap no-op).
    let mut redraw = tokio::time::interval(Duration::from_millis(200));
    redraw.tick().await;

    loop {
        state.frame = state.frame.wrapping_add(1);
        // Rebuild the theme each frame so `T` cycle takes effect on the
        // very next draw — Theme is cheap to construct.
        let theme = Theme::from_name(&state.theme_name);
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
            Some(msg) = rx.recv() => {
                handle_loop_msg(msg, &mut state, &mut last_refresh);
            }
            Some(Ok(event)) = events.next() => {
                let intent = handle_event(event, &mut state);
                match intent {
                    Intent::None => {}
                    Intent::Quit => break,
                    Intent::Refresh => {
                        trigger_refresh(handle, cli, db, cluster_id, &mut state, &tx);
                    }
                    Intent::OpenDetails => {
                        if let Some(job) = state.selected_job() {
                            let job_id = job.job_id.clone();
                            match scontrol::show(runner, &job_id).await {
                                Ok(d) => {
                                    // Fetch history stats for this job's name if we have a DB.
                                    state.details_history = match (db, cluster_id, d.job_name.as_deref()) {
                                        (Some(db), Some(cid), Some(name)) => {
                                            crate::history::job_name(&db.pool, cid, name, 30)
                                                .await
                                                .ok()
                                                .flatten()
                                        }
                                        _ => None,
                                    };
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
                                    // Kick off a background refresh — the
                                    // result lands a frame later but the
                                    // UI stays responsive.
                                    trigger_refresh(
                                        handle,
                                        cli,
                                        db,
                                        cluster_id,
                                        &mut state,
                                        &tx,
                                    );
                                }
                                Err(e) => state.last_error = Some(format!("{e}")),
                            }
                        }
                    }
                    Intent::AssistSubmit => {
                        // Build the request now (we need to copy the
                        // current jobs/partitions snapshot anyway) and
                        // spawn the LLM call so the dialog can animate
                        // its spinner while waiting.
                        if let Some(dialog) = state.assist.as_mut() {
                            let prompt = std::mem::take(&mut dialog.input);
                            dialog.in_flight = true;
                            dialog.error = None;
                            dialog.response = None;
                            let job_context = state.selected_job().map(|j| JobContext {
                                job_id: j.job_id.clone(),
                                details: state.details.clone(),
                            });
                            let req = AssistRequest {
                                prompt,
                                job_context,
                                cluster_name: handle.cluster_name.clone(),
                                jobs_snapshot: state.jobs.clone(),
                                partitions: state.partitions.clone(),
                                history_summary: None,
                            };
                            let tx2 = tx.clone();
                            let cfg = config.clone();
                            tokio::spawn(async move {
                                let result = crate::assist::assist(req, &cfg).await;
                                let _ = tx2.send(LoopMsg::AssistComplete(result)).await;
                            });
                        }
                    }
                    Intent::AssistRun(idx) => {
                        run_assisted_command(&mut state, idx);
                    }
                    Intent::ThemeChanged => {
                        if let Some(d) = db {
                            let pool = d.pool.clone();
                            let name = state.theme_name.clone();
                            tokio::spawn(async move {
                                let _ = crate::db::settings::put_theme(&pool, &name).await;
                            });
                        }
                    }
                    Intent::SettingsTest => {
                        // Fire the probe as a background task — the LLM
                        // call can take several seconds and awaiting it
                        // inline freezes the event loop (no redraws, no
                        // spinner, no keyboard).
                        let tx = tx.clone();
                        let cfg = config.clone();
                        tokio::spawn(async move {
                            let req = AssistRequest {
                                prompt: "Say hello in one short sentence.".to_string(),
                                job_context: None,
                                cluster_name: "test".to_string(),
                                jobs_snapshot: Vec::new(),
                                partitions: Vec::new(),
                                history_summary: None,
                            };
                            let result = crate::assist::assist(req, &cfg).await;
                            let _ = tx.send(LoopMsg::AssistTest(result)).await;
                        });
                    }
                    Intent::WebStart => {
                        run_web_start(&mut state, cli, config, handle, db).await;
                    }
                    Intent::SettingsSaveLlm => {
                        save_llm_config(&state.settings.llm, db).await;
                    }
                }
                if state.should_quit { break; }
            }
            _ = ticker.tick() => {
                if state.view != View::Logs {
                    trigger_refresh(handle, cli, db, cluster_id, &mut state, &tx);
                }
            }
            _ = redraw.tick() => {
                // Wake the loop so the spinner animates while a refresh
                // is in flight. The redraw happens at the top of the loop;
                // nothing else to do here.
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
        // Footer can wrap to 2 lines on narrow terminals so users always
        // see every keybind. On a 200-col terminal the second line is
        // just blank.
        let outer = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(1),
                Constraint::Length(2),
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
                // Reserve two rows at the bottom for the totals strip
                // (row 0 = aggregate line, row 1 = horizontal divider
                // separating it from the footer). Table fills the rest.
                let split = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(1), Constraint::Length(2)])
                    .split(outer[1]);
                widgets::job_table::render(frame, split[0], state, theme);
                widgets::jobs_totals::render(frame, split[1], state, theme);
                table_rect = Some(split[0]);
            }
            View::Statistics => {
                render_statistics(frame, outer[1], state, theme);
            }
            View::Settings => {
                widgets::settings::render(frame, outer[1], state, theme);
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

        widgets::footer::render(frame, outer[2], theme, state);

        if state.show_help {
            widgets::help::render(frame, frame.area(), theme);
        }
        if let Some(confirm) = &state.confirm {
            widgets::confirm::render(frame, frame.area(), confirm, theme);
        }
        if let Some(dialog) = &state.assist {
            widgets::assist::render(frame, frame.area(), dialog, theme, state.frame);
        }
    })?;
    Ok(table_rect)
}

/// Full-page statistics view: cluster overview + queue trend + wait
/// histogram + top users + partition cards.
fn render_statistics(frame: &mut ratatui::Frame<'_>, area: Rect, state: &AppState, theme: &Theme) {
    let part_rows = (state.partitions.len() as u16 + 2).clamp(3, 12);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),         // KPI strip (single-row chip bar)
            Constraint::Length(3),         // sparkline trend strip
            Constraint::Length(7),         // resources (left) + wait histogram (right)
            Constraint::Length(part_rows), // partitions
            Constraint::Min(6),            // top users (full)
        ])
        .split(area);

    widgets::kpi::render(frame, chunks[0], &state.all_jobs, &state.resources, theme);
    widgets::sparkline::render(frame, chunks[1], &state.resource_history, theme);

    let row2 = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[2]);
    widgets::resources::render(frame, row2[0], &state.resources, theme);
    widgets::wait_distribution::render(frame, row2[1], &state.all_jobs, theme);

    widgets::partitions::render(frame, chunks[3], &state.partitions, theme);
    widgets::top_users::render(frame, chunks[4], &state.all_jobs, theme);
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
            Constraint::Percentage(24),
            Constraint::Percentage(18),
            Constraint::Percentage(20),
            Constraint::Percentage(20),
            Constraint::Percentage(18),
        ])
        .split(chunks[1]);

    widgets::resources::render(frame, top[0], &state.resources, theme);
    widgets::queue::render(frame, top[1], &state.jobs, theme);
    widgets::by_user::render(frame, top[2], &state.jobs, theme);
    widgets::by_node::render(frame, top[3], &state.jobs, theme);
    widgets::ending_soon::render(frame, top[4], &state.jobs, theme);

    widgets::partitions::render(frame, chunks[2], &state.partitions, theme);
    let split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(2)])
        .split(chunks[3]);
    widgets::job_table::render(frame, split[0], state, theme);
    widgets::jobs_totals::render(frame, split[1], state, theme);

    Some(split[0])
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
    /// User pressed Enter in the assist dialog — fire the LLM call.
    AssistSubmit,
    /// User pressed 1-9 in the assist dialog — execute that proposed command.
    AssistRun(usize),
    /// User pressed `T` — persist new theme to settings.
    ThemeChanged,
    /// User pressed `t` in Settings — probe the configured LLM.
    SettingsTest,
    /// User pressed `w` — start the embedded web UI in the background.
    WebStart,
    /// User committed an edit to the LLM config — persist + apply.
    SettingsSaveLlm,
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

    // Ctrl+K: open assist dialog from any non-input view.
    if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('k')) {
        if state.assist.is_none() && state.search_input.is_none() && state.confirm.is_none() {
            state.assist = Some(AssistDialog::default());
        }
        return Intent::None;
    }

    // Assist dialog absorbs keys when open.
    if let Some(dialog) = state.assist.as_mut() {
        // Any keypress clears the transient copy-status banner, except
        // when the keypress *is* the copy itself (handled below).
        let clearing_key = !matches!(key.code, KeyCode::Char('y'));
        if clearing_key {
            dialog.copy_notice = None;
        }
        match key.code {
            KeyCode::Esc => {
                state.assist = None;
            }
            KeyCode::Enter if !dialog.in_flight && !dialog.input.is_empty() => {
                return Intent::AssistSubmit;
            }
            KeyCode::Backspace => {
                dialog.input.pop();
            }
            // `y` while the prompt input is empty and a response is
            // visible: yank the response text to the system clipboard
            // and display a confirmation banner inside the dialog so
            // the user knows the key registered (the previous silent
            // OSC-52-only path made it look broken).
            KeyCode::Char('y') if dialog.input.is_empty() && dialog.response.is_some() => {
                let text = dialog
                    .response
                    .as_ref()
                    .map(|r| r.text.clone())
                    .unwrap_or_default();
                let status = copy_to_clipboard(&text);
                dialog.copy_notice = Some(status);
            }
            KeyCode::Char(c)
                if c.is_ascii_digit() && dialog.response.is_some() && dialog.input.is_empty() =>
            {
                let idx = (c as u8 - b'0') as usize;
                if (1..=9).contains(&idx) {
                    return Intent::AssistRun(idx - 1);
                }
            }
            KeyCode::Char(c) if !dialog.in_flight => {
                dialog.input.push(c);
            }
            _ => {}
        }
        return Intent::None;
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

    // Jobs filter input — `/` in Dashboard/Jobs view opens this.
    if let Some(buf) = state.filter_input.as_mut() {
        match key.code {
            KeyCode::Esc => {
                state.filter_input = None;
            }
            KeyCode::Enter => {
                let q = state.filter_input.take().unwrap_or_default();
                state.text_filter = if q.trim().is_empty() { None } else { Some(q) };
                state.rebuild_filtered_jobs();
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
            KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => {
                return Intent::ConfirmAction;
            }
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

    // Note: last_error renders inline in the footer now, so it does not
    // gate further keypresses — only the modals above do. Errors clear
    // automatically on the next successful refresh.

    // If the user is editing an LLM config field in Settings, the
    // sub-handler should absorb every character — otherwise typing `1`,
    // `2`, `3` into a host or model name would switch views.
    if state.view == View::Settings && state.settings.edit_buffer.is_some() {
        return handle_key_settings(key, state);
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
        KeyCode::Char('3') => {
            state.view = View::Statistics;
            return Intent::None;
        }
        KeyCode::Char(',') => {
            state.view = View::Settings;
            return Intent::None;
        }
        // `w` — embedded web UI. From any non-input view, switch focus to
        // Settings (where the URL is shown) and, if not already running,
        // hot-start the server in the background.
        KeyCode::Char('w') => {
            state.view = View::Settings;
            if state.web.running.is_none() && !state.web.starting {
                state.web.starting = true;
                state.web.last_error = None;
                return Intent::WebStart;
            }
            return Intent::None;
        }
        _ => {}
    }

    match state.view {
        View::Details => handle_key_details(key, state),
        View::Logs => handle_key_logs(key, state),
        View::Settings => handle_key_settings(key, state),
        View::Dashboard | View::Jobs | View::Statistics => handle_key_jobs(key, state),
    }
}

fn handle_key_settings(key: crossterm::event::KeyEvent, state: &mut AppState) -> Intent {
    // Edit mode absorbs nearly every key — only Enter (commit) and Esc
    // (cancel) leave it. This way typing `q`, `t`, `T` into a field
    // doesn't quit the TUI or trigger an unrelated action.
    if let Some(buf) = state.settings.edit_buffer.as_mut() {
        match key.code {
            KeyCode::Esc => {
                state.settings.edit_buffer = None;
                return Intent::None;
            }
            KeyCode::Enter => {
                let value = state.settings.edit_buffer.take().unwrap_or_default();
                state
                    .settings
                    .llm
                    .set_field(state.settings.cursor, value.trim().to_string());
                return Intent::SettingsSaveLlm;
            }
            KeyCode::Backspace => {
                buf.pop();
                return Intent::None;
            }
            KeyCode::Char(c) => {
                buf.push(c);
                return Intent::None;
            }
            _ => return Intent::None,
        }
    }

    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            state.view = View::Dashboard;
            Intent::None
        }
        KeyCode::Char('T') => {
            state.theme_name = Theme::next_name(&state.theme_name).to_string();
            Intent::ThemeChanged
        }
        KeyCode::Char('t') => {
            if !state.settings.test_in_flight {
                state.settings.test_in_flight = true;
                state.settings.test_result = None;
                state.settings.test_error = None;
                Intent::SettingsTest
            } else {
                Intent::None
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if state.settings.cursor == 0 {
                state.settings.cursor = crate::app::LlmConfig::FIELDS - 1;
            } else {
                state.settings.cursor -= 1;
            }
            Intent::None
        }
        KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
            state.settings.cursor = (state.settings.cursor + 1) % crate::app::LlmConfig::FIELDS;
            Intent::None
        }
        KeyCode::Enter | KeyCode::Char('e') => {
            // Start editing the selected field with its current value.
            let current = state
                .settings
                .llm
                .field_value(state.settings.cursor)
                .to_string();
            state.settings.edit_buffer = Some(current);
            Intent::None
        }
        KeyCode::Char('?') => {
            state.show_help = true;
            Intent::None
        }
        _ => Intent::None,
    }
}

fn handle_key_jobs(key: crossterm::event::KeyEvent, state: &mut AppState) -> Intent {
    // Vim/less-style page navigation aliases:
    //   Ctrl+U / Ctrl+D — half-page (5 rows)
    //   Ctrl+B / Ctrl+F — full page (10 rows)
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('u') => {
                state.select_page_up(5);
                return Intent::None;
            }
            KeyCode::Char('d') => {
                state.select_page_down(5);
                return Intent::None;
            }
            KeyCode::Char('b') => {
                state.select_page_up(10);
                return Intent::None;
            }
            KeyCode::Char('f') => {
                state.select_page_down(10);
                return Intent::None;
            }
            _ => {}
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
            state.selected = state.display_rows.len().saturating_sub(1);
            Intent::None
        }
        KeyCode::PageDown => {
            state.select_page_down(10);
            Intent::None
        }
        KeyCode::PageUp => {
            state.select_page_up(10);
            Intent::None
        }
        KeyCode::Char('R') | KeyCode::Char('r') => Intent::Refresh,
        KeyCode::Char('a') => {
            state.filter = state.filter.cycle();
            Intent::Refresh
        }
        KeyCode::Char('/') => {
            state.filter_input = Some(state.text_filter.clone().unwrap_or_default());
            Intent::None
        }
        KeyCode::Tab => {
            state.group_by = state.group_by.cycle();
            // Reset collapse so the new grouping starts fully expanded.
            state.collapsed_groups.clear();
            state.rebuild_display_rows();
            Intent::None
        }
        KeyCode::Enter | KeyCode::Char('d') => {
            // Enter on a group header toggles collapse; otherwise → details.
            if state.toggle_selected_group() {
                Intent::None
            } else {
                Intent::OpenDetails
            }
        }
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
        KeyCode::Char('T') => {
            state.theme_name = Theme::next_name(&state.theme_name).to_string();
            Intent::ThemeChanged
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
            state.details_history = None;
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
    let MouseEvent {
        kind, column, row, ..
    } = m;
    let Some(table_rect) = state.table_rect else {
        return;
    };
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
            if idx < state.display_rows.len() {
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

/// Spawn background squeue + sinfo refreshes. Results are delivered through
/// `tx` and handled in [`handle_loop_msg`]. Idempotent — if a refresh is
/// already in flight, that kind is skipped.
fn trigger_refresh(
    handle: &RunnerHandle,
    cli: &Cli,
    db: Option<&Db>,
    cluster_id: Option<i64>,
    state: &mut AppState,
    tx: &tokio::sync::mpsc::Sender<LoopMsg>,
) {
    if !state.refresh.jobs_in_flight {
        state.refresh.jobs_in_flight = true;
        let runner = handle.runner.clone();
        let opts = squeue_opts(cli, &state.filter);
        let tx = tx.clone();
        let pool = db.map(|d| d.pool.clone());
        let cluster_name = handle.cluster_name.clone();
        tokio::spawn(async move {
            let start = std::time::Instant::now();
            tracing::info!(cluster = %cluster_name, me = opts.me, "squeue refresh start");
            let result = squeue::list(runner.as_ref(), &opts).await;
            let elapsed_ms = start.elapsed().as_millis();
            match &result {
                Ok(jobs) => tracing::info!(
                    cluster = %cluster_name,
                    elapsed_ms = elapsed_ms as u64,
                    jobs = jobs.len(),
                    "squeue refresh ok"
                ),
                Err(e) => tracing::warn!(
                    cluster = %cluster_name,
                    elapsed_ms = elapsed_ms as u64,
                    error = %e,
                    "squeue refresh failed"
                ),
            }
            if let (Ok(jobs), Some(p), Some(cid)) = (&result, &pool, cluster_id) {
                if let Err(e) = snapshots::write_jobs(p, cid, jobs).await {
                    tracing::warn!(error = %e, "job_snapshots write failed");
                }
            }
            let _ = tx.send(LoopMsg::Jobs(result)).await;
        });
    }
    if !state.refresh.sinfo_in_flight {
        state.refresh.sinfo_in_flight = true;
        let runner = handle.runner.clone();
        let tx = tx.clone();
        let pool = db.map(|d| d.pool.clone());
        let cluster_name = handle.cluster_name.clone();
        tokio::spawn(async move {
            let start = std::time::Instant::now();
            tracing::info!(cluster = %cluster_name, "sinfo refresh start");
            let result = sinfo::list_partitions(runner.as_ref()).await;
            let elapsed_ms = start.elapsed().as_millis();
            match &result {
                Ok(parts) => tracing::info!(
                    cluster = %cluster_name,
                    elapsed_ms = elapsed_ms as u64,
                    partitions = parts.len(),
                    "sinfo refresh ok"
                ),
                Err(e) => tracing::warn!(
                    cluster = %cluster_name,
                    elapsed_ms = elapsed_ms as u64,
                    error = %e,
                    "sinfo refresh failed"
                ),
            }
            if let (Ok(parts), Some(p), Some(cid)) = (&result, &pool, cluster_id) {
                if let Err(e) = snapshots::write_resources(p, cid, parts).await {
                    tracing::warn!(error = %e, "resource_snapshots write failed");
                }
            }
            let _ = tx.send(LoopMsg::Partitions(result)).await;
        });
    }
}

/// Apply the result of a finished background task to the app state.
fn handle_loop_msg(
    msg: LoopMsg,
    state: &mut AppState,
    last_refresh: &mut Option<chrono::DateTime<chrono::Utc>>,
) {
    match msg {
        LoopMsg::Jobs(Ok(jobs)) => {
            state.refresh.jobs_in_flight = false;
            state.all_jobs = jobs;
            state.rebuild_filtered_jobs();
            state.last_error = None;
            *last_refresh = Some(chrono::Utc::now());
        }
        LoopMsg::Jobs(Err(e)) => {
            state.refresh.jobs_in_flight = false;
            state.last_error = Some(format!("{e}"));
        }
        LoopMsg::Partitions(Ok(parts)) => {
            state.refresh.sinfo_in_flight = false;
            let resources = ClusterResources::from_partitions(&parts);
            state.push_resource_sample(ResourceSample::from(chrono::Utc::now(), &resources));
            state.resources = resources;
            state.partitions = parts;
        }
        LoopMsg::Partitions(Err(e)) => {
            state.refresh.sinfo_in_flight = false;
            tracing::warn!(error = %e, "sinfo refresh failed");
        }
        LoopMsg::AssistTest(result) => {
            state.settings.test_in_flight = false;
            match result {
                Ok(r) => {
                    state.settings.test_result =
                        Some(format!("[{} · {}] {}", r.provider, r.model, r.text));
                    state.settings.test_error = None;
                }
                Err(e) => {
                    state.settings.test_error = Some(format!("{e}"));
                    state.settings.test_result = None;
                }
            }
        }
        LoopMsg::AssistComplete(result) => {
            if let Some(dialog) = state.assist.as_mut() {
                dialog.in_flight = false;
                match result {
                    Ok(r) => {
                        dialog.response = Some(r);
                        dialog.error = None;
                    }
                    Err(e) => {
                        dialog.error = Some(format!("{e}"));
                        dialog.response = None;
                    }
                }
            }
        }
    }
}

/// Start the embedded web UI listener in-process. Triggered by `w` in
/// the TUI. On success, writes the bound URL+token into `state.web` so
/// the Settings view can render them; on failure, records the error
/// message instead of crashing the TUI. The server task itself runs for
/// the rest of the process — there's no way to stop it from the TUI
/// short of quitting.
async fn run_web_start(
    state: &mut AppState,
    cli: &Cli,
    config: &Config,
    handle: &RunnerHandle,
    db: Option<&Db>,
) {
    state.web.starting = false;
    let opts = match crate::web::WebOptions::from_cli(None, None, false, true) {
        Ok(o) => o,
        Err(e) => {
            state.web.last_error = Some(format!("{e}"));
            return;
        }
    };
    let refresh_secs = cli.refresh.unwrap_or(config.ui.refresh_seconds).max(1);
    let result = crate::web::spawn(
        config.clone(),
        handle.clone(),
        db.cloned(),
        opts,
        refresh_secs,
    )
    .await;
    match result {
        Ok(h) => {
            let url = format!("http://{}/?token={}", h.addr, h.token);
            state.web.running = Some(crate::app::WebUiInfo {
                url,
                token: h.token,
                addr: h.addr.to_string(),
                readonly: false,
            });
            state.web.last_error = None;
            // Dropping the JoinHandle detaches the task; the axum server
            // keeps running in the background until the process exits.
            drop(h.task);
        }
        Err(e) => {
            state.web.last_error = Some(format!("{e}"));
        }
    }
}

/// Mirror the LLM config values into process env so the assist
/// providers — which read env vars at call time — pick up changes
/// made through the Settings view. `set_var` is `unsafe` in Rust
/// 2024; calling it before any other thread touches env is fine
/// because nothing in slurmdash spawns workers that read these vars
/// concurrently.
fn apply_llm_to_env(llm: &crate::app::LlmConfig) {
    // Safety: single-threaded with respect to env reads of these keys.
    // Callers are TUI startup + the SettingsSaveLlm dispatcher; no
    // background task reads SLURMDASH_LLM_PROVIDER / OLLAMA_* /
    // ANTHROPIC_MODEL at the moment we write them.
    unsafe {
        std::env::set_var("SLURMDASH_LLM_PROVIDER", &llm.provider);
        std::env::set_var("OLLAMA_HOST", &llm.ollama_host);
        std::env::set_var("OLLAMA_MODEL", &llm.ollama_model);
        std::env::set_var("ANTHROPIC_MODEL", &llm.anthropic_model);
    }
}

/// Persist edited LLM config to the DB settings KV and rewrite env
/// vars so the next `t` test / Ctrl+K assist call sees the new
/// values. Silently no-ops if no DB is configured (env-only mode).
async fn save_llm_config(llm: &crate::app::LlmConfig, db: Option<&Db>) {
    apply_llm_to_env(llm);
    if let Some(d) = db {
        let payload = crate::db::settings::AssistSettings {
            provider: Some(llm.provider.clone()),
            ollama_host: Some(llm.ollama_host.clone()),
            ollama_model: Some(llm.ollama_model.clone()),
            anthropic_model: Some(llm.anthropic_model.clone()),
        };
        let _ = crate::db::settings::put_assist(&d.pool, &payload).await;
    }
}

/// Push `text` to the user's system clipboard. Tries native clipboard
/// tools in priority order (matching the host platform) and falls back
/// to the OSC 52 escape sequence for terminal-resident copy. Returns a
/// short status string indicating what worked (or didn't) so the UI can
/// surface it to the user — the previous OSC-only path was silent and
/// looked broken whenever the terminal stripped OSC 52 (tmux without
/// `set -g set-clipboard on`, GNOME Terminal, etc.).
fn copy_to_clipboard(text: &str) -> String {
    let candidates: Vec<(&str, &[&str])> = if cfg!(target_os = "macos") {
        vec![("pbcopy", &[][..])]
    } else if cfg!(target_os = "windows") {
        vec![("clip", &[][..])]
    } else if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        vec![
            ("wl-copy", &[][..]),
            ("xclip", &["-selection", "clipboard"][..]),
            ("xsel", &["-b", "-i"][..]),
        ]
    } else {
        vec![
            ("xclip", &["-selection", "clipboard"][..]),
            ("xsel", &["-b", "-i"][..]),
            ("wl-copy", &[][..]),
        ]
    };
    for (tool, args) in &candidates {
        if try_spawn_copy(tool, args, text).is_ok() {
            return format!("✓ copied {} chars via {tool}", text.chars().count());
        }
    }
    // Fall back to OSC 52 — silent in many terminals but free.
    let b64 = base64_encode(text.as_bytes());
    let mut stdout = std::io::stdout().lock();
    use std::io::Write;
    let _ = write!(stdout, "\x1b]52;c;{b64}\x07");
    let _ = stdout.flush();
    let tools: Vec<&str> = candidates.iter().map(|(t, _)| *t).collect();
    format!(
        "↗ sent via OSC 52 (terminal-dependent). install one of: {} for native copy",
        tools.join(" / ")
    )
}

fn try_spawn_copy(tool: &str, args: &[&str], text: &str) -> std::io::Result<()> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut child = Command::new(tool)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(text.as_bytes())?;
        drop(stdin);
    }
    let status = child.wait()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!("{tool} exited {status}")))
    }
}

/// Minimal standard-alphabet Base64 encoder. Pulled inline to avoid
/// adding a dependency just for the clipboard path.
fn base64_encode(bytes: &[u8]) -> String {
    const ALPHA: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    let mut i = 0;
    while i + 2 < bytes.len() {
        let n = ((bytes[i] as u32) << 16) | ((bytes[i + 1] as u32) << 8) | (bytes[i + 2] as u32);
        out.push(ALPHA[((n >> 18) & 0x3f) as usize] as char);
        out.push(ALPHA[((n >> 12) & 0x3f) as usize] as char);
        out.push(ALPHA[((n >> 6) & 0x3f) as usize] as char);
        out.push(ALPHA[(n & 0x3f) as usize] as char);
        i += 3;
    }
    let rem = bytes.len() - i;
    if rem == 1 {
        let n = (bytes[i] as u32) << 16;
        out.push(ALPHA[((n >> 18) & 0x3f) as usize] as char);
        out.push(ALPHA[((n >> 12) & 0x3f) as usize] as char);
        out.push('=');
        out.push('=');
    } else if rem == 2 {
        let n = ((bytes[i] as u32) << 16) | ((bytes[i + 1] as u32) << 8);
        out.push(ALPHA[((n >> 18) & 0x3f) as usize] as char);
        out.push(ALPHA[((n >> 12) & 0x3f) as usize] as char);
        out.push(ALPHA[((n >> 6) & 0x3f) as usize] as char);
        out.push('=');
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn run_assisted_command(state: &mut AppState, idx: usize) {
    let cmd_opt = state
        .assist
        .as_ref()
        .and_then(|d| d.response.as_ref())
        .and_then(|r| r.commands.get(idx).cloned());
    let Some(cmd) = cmd_opt else { return };

    let action_kind = match &cmd.kind {
        ProposedKind::Cancel { .. } => Some(ActionKind::Cancel),
        ProposedKind::Hold { .. } => Some(ActionKind::Hold),
        ProposedKind::Release { .. } => Some(ActionKind::Release),
        ProposedKind::Requeue { .. } => Some(ActionKind::Requeue),
        ProposedKind::Sbatch { .. } | ProposedKind::Shell { .. } => None,
    };
    let job_id = match &cmd.kind {
        ProposedKind::Cancel { job_id }
        | ProposedKind::Hold { job_id }
        | ProposedKind::Release { job_id }
        | ProposedKind::Requeue { job_id } => Some(job_id.clone()),
        _ => None,
    };
    let (Some(kind), Some(job_id)) = (action_kind, job_id) else {
        state.last_error = Some("assisted sbatch/shell commands are not yet supported".to_string());
        return;
    };

    // Close assist dialog and route through the normal confirm modal so the
    // user gets the exact-command preview and a definitive y/n.
    state.assist = None;
    state.confirm = Some(Confirm {
        kind,
        job_id: job_id.clone(),
        preview: cmd.preview.clone(),
    });
}

/// Translate the runtime filter state plus CLI-fixed flags into squeue
/// options. Computed per-fetch so the `a` toggle takes effect on the next
/// refresh.
fn squeue_opts(cli: &Cli, filter: &FilterMode) -> squeue::Options {
    let (me, user) = match filter {
        FilterMode::Me => (true, None),
        FilterMode::All => (false, None),
        FilterMode::User(u) => (false, Some(u.clone())),
    };
    squeue::Options {
        me,
        user,
        partition: cli.partition.clone(),
        state: cli.state.clone(),
        extra_args: Vec::new(),
    }
}
