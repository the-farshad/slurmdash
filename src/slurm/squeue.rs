use anyhow::Result;

use super::model::Job;
use super::parse::{SQUEUE_FORMAT, parse_squeue_text};
use crate::ssh::Runner;

/// Options for a single `squeue` invocation.
#[derive(Debug, Clone, Default)]
pub struct Options {
    pub me: bool,
    pub user: Option<String>,
    pub partition: Option<String>,
    pub state: Option<String>,
    pub extra_args: Vec<String>,
}

/// Run `squeue` and parse the result.
///
/// Phase 1 uses the text format (`--format=`) for maximum compatibility. JSON
/// (`--json`) on Slurm 20.11+ will be added in Phase 2 alongside version
/// detection.
pub async fn list(runner: &dyn Runner, opts: &Options) -> Result<Vec<Job>> {
    let mut argv: Vec<String> = vec!["--noheader".into(), format!("--format={SQUEUE_FORMAT}")];

    if opts.me {
        argv.push("--me".into());
    }
    if let Some(u) = &opts.user {
        argv.push("--user".into());
        argv.push(u.clone());
    }
    if let Some(p) = &opts.partition {
        argv.push("--partition".into());
        argv.push(p.clone());
    }
    if let Some(s) = &opts.state {
        argv.push("--states".into());
        argv.push(s.clone());
    }
    for x in &opts.extra_args {
        argv.push(x.clone());
    }

    let args_ref: Vec<&str> = argv.iter().map(|s| s.as_str()).collect();
    let out = runner.run("squeue", &args_ref).await?.check("squeue")?;
    Ok(parse_squeue_text(&out.stdout))
}
