use anyhow::{Context, Result};
use futures::future::BoxFuture;
use openssh::{KnownHosts, Session, SessionBuilder};

use super::{CommandOutput, Runner};
use crate::config::ClusterProfile;

/// Persistent SSH session to a cluster login node.
///
/// Uses the system `ssh` binary with ControlMaster multiplexing — all the
/// normal OpenSSH features (key agents, `~/.ssh/config`, ProxyJump,
/// `known_hosts`) apply automatically.
pub struct RemoteRunner {
    session: Session,
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
            session,
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

    fn description(&self) -> String {
        self.description.clone()
    }
}
