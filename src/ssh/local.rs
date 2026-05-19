use anyhow::{Context, Result};
use futures::future::BoxFuture;
use tokio::process::Command;

use super::{CommandOutput, Runner};

/// Runs Slurm commands directly on this machine, no SSH.
///
/// Used when a cluster profile sets `local = true`, or when the user has not
/// configured any cluster and Slurm CLI tools are on `PATH` locally.
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

    fn description(&self) -> String {
        "local".to_string()
    }
}
