use anyhow::Result;

use crate::ssh::Runner;

/// Command preview rendered before execution, shown in the confirm modal.
pub fn preview(job_id: &str) -> String {
    format!("scancel {job_id}")
}

pub async fn cancel(runner: &dyn Runner, job_id: &str) -> Result<()> {
    let out = runner.run("scancel", &[job_id]).await?.check("scancel")?;
    tracing::debug!(job_id, stderr = %out.stderr, "scancel ok");
    Ok(())
}
