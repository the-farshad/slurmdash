use std::process::Stdio;

use anyhow::{Context, Result};
use futures::future::BoxFuture;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

use super::{CommandOutput, LineStream, Runner};

pub struct LocalRunner;

impl LocalRunner {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LocalRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl Runner for LocalRunner {
    fn run<'a>(
        &'a self,
        program: &'a str,
        args: &'a [&'a str],
    ) -> BoxFuture<'a, Result<CommandOutput>> {
        Box::pin(async move {
            let output = Command::new(program)
                .args(args)
                .output()
                .await
                .with_context(|| format!("spawning {program}"))?;
            Ok(CommandOutput {
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
                status: output.status.code().unwrap_or(-1),
            })
        })
    }

    fn stream<'a>(
        &'a self,
        program: &'a str,
        args: &'a [&'a str],
    ) -> BoxFuture<'a, Result<LineStream>> {
        Box::pin(async move {
            let mut child = Command::new(program)
                .args(args)
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .with_context(|| format!("spawning {program}"))?;
            let stdout = child
                .stdout
                .take()
                .context("child has no stdout pipe")?;
            let mut lines = BufReader::new(stdout).lines();
            let (tx, rx) = mpsc::channel(1024);
            let join = tokio::spawn(async move {
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
        "local".to_string()
    }
}
