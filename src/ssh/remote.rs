use std::sync::Arc;

use anyhow::{Context, Result};
use futures::future::BoxFuture;
use openssh::{KnownHosts, Session, SessionBuilder, Stdio};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;

use super::{CommandOutput, LineStream, Runner};
use crate::config::ClusterProfile;

/// Persistent SSH session to a cluster login node.
///
/// Uses the system `ssh` binary with ControlMaster multiplexing — all the
/// normal OpenSSH features (key agents, `~/.ssh/config`, ProxyJump,
/// `known_hosts`) apply automatically.
///
/// The `Session` is held in an `Arc` so streaming tasks can clone it into
/// `'static` spawned futures (a `RemoteChild` borrows from its `Session`,
/// so the spawned task needs to keep the session alive itself).
pub struct RemoteRunner {
    session: Arc<Session>,
    description: String,
}

impl RemoteRunner {
    pub async fn connect(host: &str, profile: ClusterProfile) -> Result<Self> {
        let target = match (&profile.user, host) {
            (Some(u), h) => format!("{u}@{h}"),
            (None, h) => h.to_string(),
        };

        let mut builder = SessionBuilder::default();
        builder.known_hosts_check(KnownHosts::Strict);
        if let Some(port) = profile.port {
            builder.port(port);
        }
        if let Some(key) = &profile.ssh_key {
            builder.keyfile(key);
        }

        let session = builder
            .connect(&target)
            .await
            .with_context(|| format!("connecting to {target}"))?;

        Ok(Self {
            session: Arc::new(session),
            description: target,
        })
    }
}

impl Runner for RemoteRunner {
    fn run<'a>(
        &'a self,
        program: &'a str,
        args: &'a [&'a str],
    ) -> BoxFuture<'a, Result<CommandOutput>> {
        Box::pin(async move {
            let mut cmd = self.session.command(program);
            for a in args {
                cmd.arg(a);
            }
            let out = cmd
                .output()
                .await
                .with_context(|| format!("running {program} over ssh"))?;
            Ok(CommandOutput {
                stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
                status: out.status.code().unwrap_or(-1),
            })
        })
    }

    fn stream<'a>(
        &'a self,
        program: &'a str,
        args: &'a [&'a str],
    ) -> BoxFuture<'a, Result<LineStream>> {
        let session = self.session.clone();
        let program = program.to_string();
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();

        Box::pin(async move {
            let (tx, rx) = mpsc::channel(1024);
            let join = tokio::spawn(async move {
                let mut cmd = session.command(&program);
                for a in &args {
                    cmd.arg(a);
                }
                cmd.stdout(Stdio::piped());
                cmd.stderr(Stdio::null());
                let Ok(mut child) = cmd.spawn().await else {
                    return;
                };
                let Some(stdout) = child.stdout().take() else {
                    return;
                };
                let mut lines = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    if tx.send(line).await.is_err() {
                        break;
                    }
                }
                let _ = child.wait().await;
            });
            Ok(LineStream { rx, join })
        })
    }

    fn description(&self) -> String {
        self.description.clone()
    }
}
