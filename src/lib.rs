pub mod actions;
pub mod app;
pub mod assist;
pub mod cli;
pub mod config;
pub mod db;
pub mod error;
pub mod history;
pub mod slurm;
pub mod ssh;
pub mod tui;
pub mod web;

use anyhow::Result;
use clap::Parser;

pub async fn run() -> Result<()> {
    let _log_guard = init_tracing();
    let cli = cli::Cli::parse();
    cli::dispatch(cli).await
}

/// Initialize tracing. Logs land in
/// `~/.local/share/slurmdash/slurmdash.log.<date>` so they don't corrupt
/// the TUI's alternate-screen on stdout. Override the level filter with
/// the `SLURMDASH_LOG` env var (e.g. `SLURMDASH_LOG=debug`).
///
/// The returned guard must outlive the program; callers drop it at exit.
fn init_tracing() -> Option<tracing_appender::non_blocking::WorkerGuard> {
    use tracing_appender::{non_blocking, rolling};
    use tracing_subscriber::{EnvFilter, fmt, prelude::*};

    let filter = EnvFilter::try_from_env("SLURMDASH_LOG")
        .unwrap_or_else(|_| EnvFilter::new("warn,slurmdash=info"));

    let log_dir =
        directories::ProjectDirs::from("", "", "slurmdash").map(|d| d.data_dir().to_path_buf());
    if let Some(dir) = &log_dir {
        let _ = std::fs::create_dir_all(dir);
    }

    let (writer, guard) = match log_dir {
        Some(dir) => {
            let appender = rolling::daily(dir, "slurmdash.log");
            non_blocking(appender)
        }
        None => non_blocking(std::io::stderr()),
    };

    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(
            fmt::layer()
                .with_writer(writer)
                .with_target(false)
                .with_ansi(false),
        )
        .try_init();

    Some(guard)
}
