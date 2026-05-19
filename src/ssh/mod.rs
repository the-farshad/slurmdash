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

use anyhow::{Context, Result};
use futures::future::BoxFuture;

use crate::cli::Cli;
use crate::config::{ClusterProfile, Config};

/// Outcome of a single command invocation.
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

/// Something that can execute a remote (or local) shell command.
///
/// Returns a boxed future so the trait is object-safe.
pub trait Runner: Send + Sync {
    fn run<'a>(
        &'a self,
        program: &'a str,
        args: &'a [&'a str],
    ) -> BoxFuture<'a, Result<CommandOutput>>;

    /// Short description used in audit logs and the UI status bar.
    fn description(&self) -> String;
}

/// A connected runner plus enough metadata for the DB layer to key snapshots
/// and audit-log entries by cluster.
pub struct RunnerHandle {
    pub runner: Box<dyn Runner>,
    pub cluster_name: String,
    pub is_local: bool,
}

/// Construct a runner for the cluster selected by CLI flags + config.
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
            runner: Box::new(local::LocalRunner::new()),
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
        runner: Box::new(runner),
        cluster_name: name,
        is_local: false,
    })
}
