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
    init_tracing()?;
    let cli = cli::Cli::parse();
    cli::dispatch(cli).await
}

fn init_tracing() -> Result<()> {
    use tracing_subscriber::{EnvFilter, fmt, prelude::*};

    let filter = EnvFilter::try_from_env("SLURMDASH_LOG")
        .unwrap_or_else(|_| EnvFilter::new("warn,slurmdash=info"));

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_writer(std::io::stderr).with_target(false))
        .init();

    Ok(())
}
