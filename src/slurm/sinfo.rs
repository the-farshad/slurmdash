//! sinfo wrapper — placeholder for Phase 2 resource dashboard.

use anyhow::Result;

use crate::ssh::Runner;

#[allow(dead_code)]
pub async fn raw(runner: &dyn Runner) -> Result<String> {
    let out = runner.run("sinfo", &[]).await?.check("sinfo")?;
    Ok(out.stdout)
}
