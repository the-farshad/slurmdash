//! Snapshot writers for job_snapshots and resource_snapshots tables.
//! Phase 1 stub — wired into the refresh loop in Phase 1.12.

use anyhow::Result;

use crate::slurm::model::Job;

#[allow(dead_code)]
pub async fn write_jobs(_cluster: &str, _jobs: &[Job]) -> Result<()> {
    Ok(())
}
