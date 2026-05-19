//! SSH and local command execution.
//!
//! All Slurm interactions go through a [`Runner`]. There are two
//! implementations: [`local::LocalRunner`] (runs commands directly on this
//! machine, used for development and for clusters with `local = true`) and
//! [`remote::RemoteRunner`] (wraps the system `ssh` binary with ControlMaster
//! multiplexing via the `openssh` crate).

pub mod local;
pub mod remote;
pub mod tail;

use std::sync::Arc;

use anyhow::{Context, Result};
use futures::future::BoxFuture;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::cli::Cli;
use crate::config::{ClusterProfile, Config};

#[derive(Debug, Clone)]
pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub status: i32,
}

impl CommandOutput {
    pub fn ok(&self) -> bool {
        self.status == 0
    }

    pub fn check(self, cmd: &str) -> std::result::Result<Self, crate::error::Error> {
        if self.ok() {
            Ok(self)
        } else {
            Err(crate::error::Error::SlurmCommand {
                code: self.status,
                stderr: format!("{cmd}: {}", self.stderr.trim()),
            })
        }
    }
}

/// A live stream of stdout lines from a long-running remote command (used
/// for `tail -F`). The `join` handle is kept so the streaming task is
/// awaited on drop.
pub struct LineStream {
    pub rx: mpsc::Receiver<String>,
    #[allow(dead_code)]
    pub join: JoinHandle<()>,
}

pub trait Runner: Send + Sync {
    fn run<'a>(
        &'a self,
        program: &'a str,
        args: &'a [&'a str],
    ) -> BoxFuture<'a, Result<CommandOutput>>;

    fn stream<'a>(
        &'a self,
        program: &'a str,
        args: &'a [&'a str],
    ) -> BoxFuture<'a, Result<LineStream>>;

    fn description(&self) -> String;
}

pub struct RunnerHandle {
    pub runner: Arc<dyn Runner>,
    pub cluster_name: String,
    pub is_local: bool,
}

pub async fn build_runner(cli: &Cli, config: &Config) -> Result<RunnerHandle> {
    let (profile, name) = if cli.host.is_some() || cli.user.is_some() {
        let p = ClusterProfile {
            local: false,
            host: cli.host.clone(),
            user: cli.user.clone(),
            port: cli.port,
            ssh_key: cli.ssh_key.clone(),
            ..Default::default()
        };
        let name = match (&p.user, &p.host) {
            (Some(u), Some(h)) => format!("{u}@{h}"),
            (None, Some(h)) => h.clone(),
            _ => "remote".to_string(),
        };
        (p, name)
    } else {
        let name = cli.cluster.clone().unwrap_or_else(|| "default".to_string());
        let profile = config
            .resolve_cluster(cli.cluster.as_deref())
            .context("resolving cluster profile")?;
        (profile, name)
    };

    if profile.local {
        return Ok(RunnerHandle {
            runner: Arc::new(local::LocalRunner::new()),
            cluster_name: name,
            is_local: true,
        });
    }

    let host = profile
        .host
        .clone()
        .context("cluster profile has no `host` and is not marked `local = true`")?;

    let runner = remote::RemoteRunner::connect(&host, profile).await?;
    Ok(RunnerHandle {
        runner: Arc::new(runner),
        cluster_name: name,
        is_local: false,
    })
}
