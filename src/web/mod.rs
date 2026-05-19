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

    let token = Uuid::new_v4().to_string();
    let refresh_secs = cli.refresh.unwrap_or(config.ui.refresh_seconds).max(1);

    let state = Arc::new(WebState::new(
        Arc::new(handle),
        db,
        config.clone(),
        token.clone(),
        opts.readonly,
    ));

    state.bootstrap_cluster_row().await;

    // Background refresher
    {
        let state = state.clone();
        tokio::spawn(async move {
            refresh::run(state, Duration::from_secs(refresh_secs)).await;
        });
    }

    let app = Router::new()
        .route("/", get(assets::index))
        .route("/style.css", get(assets::style))
        .route("/app.js", get(assets::app_js))
        .route("/api/dashboard", get(api::dashboard))
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

    println!();
    println!("slurmdash web");
    println!("  url:     http://{}/?token={}", actual, token);
    println!("  cluster: {}", state.handle.cluster_name);
    println!(
        "  mode:    {}",
        if opts.readonly { "readonly" } else { "read/write" }
    );
    println!("  press Ctrl+C to stop");
    println!();

    let _ = cli; // future: support --offline etc.

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("axum::serve")?;

    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    eprintln!("\nshutting down…");
}
