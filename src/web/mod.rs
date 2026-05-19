//! Local web UI.
//!
//! Serves the same dashboard as the TUI through a browser, on a
//! user-selected loopback port. Same backend modules — `slurm`, `ssh`, `db`,
//! `actions` — feed both the TUI and the web handlers; the only
//! browser-specific code is HTTP routing and the embedded HTML/CSS/JS.

pub mod api;
pub mod assets;
pub mod auth;
pub mod refresh;
pub mod state;

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use axum::Router;
use axum::routing::{get, post};
use uuid::Uuid;

use crate::cli::Cli;
use crate::config::Config;
use crate::db::Db;
use crate::ssh::RunnerHandle;
use crate::web::state::WebState;

#[derive(Debug, Clone)]
pub struct WebOptions {
    pub host: IpAddr,
    pub port: u16,
    pub readonly: bool,
    pub open_browser: bool,
}

impl WebOptions {
    pub fn from_cli(
        host: Option<String>,
        port: Option<u16>,
        readonly: bool,
        no_open_browser: bool,
    ) -> Result<Self> {
        let host = match host.as_deref() {
            Some(h) => h.parse().with_context(|| format!("parsing --host {h}"))?,
            None => IpAddr::from([127, 0, 0, 1]),
        };
        Ok(Self {
            host,
            port: port.unwrap_or(8080),
            readonly,
            open_browser: !no_open_browser,
        })
    }
}

/// Handle for a running web server bound to a concrete address.
///
/// Returned by [`spawn`] so callers can read the bound URL and the
/// generated auth token immediately, without waiting for the server to
/// shut down. The server keeps running until the process exits (or the
/// task is aborted via the returned join handle).
pub struct WebHandle {
    pub addr: SocketAddr,
    pub token: String,
    pub task: tokio::task::JoinHandle<Result<()>>,
}

/// Bind, register routes, and spawn the axum server as a background
/// tokio task. Returns once the listener is bound, so callers learn the
/// concrete (host, port) and token without blocking. The actual
/// `axum::serve` future runs inside `task` for the rest of the process.
pub async fn spawn(
    config: Config,
    handle: RunnerHandle,
    db: Option<Db>,
    opts: WebOptions,
    refresh_seconds: u64,
) -> Result<WebHandle> {
    let token = Uuid::new_v4().to_string();

    let state = Arc::new(WebState::new(
        Arc::new(handle),
        db,
        config,
        token.clone(),
        opts.readonly,
    ));

    state.bootstrap_cluster_row().await;

    {
        let state = state.clone();
        tokio::spawn(async move {
            refresh::run(state, Duration::from_secs(refresh_seconds.max(1))).await;
        });
    }

    let app = Router::new()
        .route("/", get(assets::index))
        .route("/style.css", get(assets::style))
        .route("/app.js", get(assets::app_js))
        .route("/api/dashboard", get(api::dashboard))
        .route("/api/history", get(api::history))
        .route("/api/jobs/:job_id", get(api::job_details))
        .route("/api/jobs/:job_id/cancel", post(api::cancel))
        .route("/api/jobs/:job_id/hold", post(api::hold))
        .route("/api/jobs/:job_id/release", post(api::release))
        .route("/api/jobs/:job_id/requeue", post(api::requeue))
        .route("/api/assist", post(api::assist))
        .with_state(state.clone());

    let addr = SocketAddr::new(opts.host, opts.port);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("binding to {addr}"))?;
    let actual = listener.local_addr().unwrap_or(addr);

    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.context("axum::serve")?;
        Ok(())
    });

    Ok(WebHandle {
        addr: actual,
        token,
        task,
    })
}

pub async fn run(
    cli: Cli,
    config: Config,
    handle: RunnerHandle,
    db: Option<Db>,
    opts: WebOptions,
) -> Result<()> {
    if !opts.host.is_loopback() {
        eprintln!(
            "warning: binding to {} exposes slurmdash beyond this machine.",
            opts.host
        );
        eprintln!("         destructive actions will still require token auth.");
    }
    let readonly = opts.readonly;
    let cluster_name = handle.cluster_name.clone();
    let refresh_secs = cli.refresh.unwrap_or(config.ui.refresh_seconds).max(1);

    let web = spawn(config, handle, db, opts, refresh_secs).await?;

    println!();
    println!("slurmdash web");
    println!("  url:     http://{}/?token={}", web.addr, web.token);
    println!("  cluster: {cluster_name}");
    println!(
        "  mode:    {}",
        if readonly { "readonly" } else { "read/write" }
    );
    println!("  press Ctrl+C to stop");
    println!();

    let _ = cli;

    tokio::select! {
        res = web.task => res.map_err(|e| anyhow::anyhow!(e))?,
        _ = shutdown_signal() => Ok(()),
    }
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    eprintln!("\nshutting down…");
}
