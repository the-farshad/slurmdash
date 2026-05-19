use anyhow::Result;

use super::model::Partition;
use super::parse::parse_sinfo_text;
use crate::ssh::Runner;

/// Format string passed to `sinfo --format=`. Order must match
/// [`parse_sinfo_text`].
///
/// Fields: Partition|NodeAIOT|CPUsAIOT|MemoryMB|Gres
pub const SINFO_FORMAT: &str = "%P|%F|%C|%m|%G";

pub async fn list_partitions(runner: &dyn Runner) -> Result<Vec<Partition>> {
    let argv = ["--noheader", &format!("--format={SINFO_FORMAT}")];
    let argv_ref: Vec<&str> = argv.iter().map(|s| *s as &str).collect();
    let out = runner.run("sinfo", &argv_ref).await?.check("sinfo")?;
    Ok(parse_sinfo_text(&out.stdout))
}
