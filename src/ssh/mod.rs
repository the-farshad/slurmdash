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

#[derive(Clone)]
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

    // The `openssh` crate spawns the system `ssh` binary — surface a clear
    // hint up front instead of an obscure "No such file or directory" deep
    // inside the connect path. Skipped on local mode (handled above).
    check_ssh_available().context("checking for ssh in PATH")?;

    let runner = remote::RemoteRunner::connect(&host, profile).await?;
    Ok(RunnerHandle {
        runner: Arc::new(runner),
        cluster_name: name,
        is_local: false,
    })
}

/// Verify the system `ssh` client is installed. We don't try to run it;
/// just confirm a candidate path exists in `PATH`. Helpful because on a
/// fresh box `cargo install slurmdash` succeeds but the resulting binary
/// fails at the first connect with an opaque "spawning ssh: …" error.
fn check_ssh_available() -> Result<()> {
    if ssh_in_path() {
        return Ok(());
    }
    anyhow::bail!(
        "could not find `ssh` in $PATH.\n\n\
         slurmdash uses the system OpenSSH client to reach remote clusters.\n\
         Install it with one of:\n  \
         - Debian / Ubuntu:  sudo apt install openssh-client\n  \
         - Fedora / RHEL:    sudo dnf install openssh-clients\n  \
         - Arch:             sudo pacman -S openssh\n  \
         - macOS:            already included with the system\n  \
         - Windows:          install OpenSSH via Settings → Apps → Optional features"
    )
}

fn ssh_in_path() -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    let exe_names: &[&str] = if cfg!(windows) {
        &["ssh.exe", "ssh"]
    } else {
        &["ssh"]
    };
    for dir in std::env::split_paths(&path) {
        for name in exe_names {
            if dir.join(name).is_file() {
                return true;
            }
        }
    }
    false
}
