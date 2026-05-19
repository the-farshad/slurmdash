use anyhow::Result;

use super::model::JobDetails;
use super::parse::parse_scontrol_show_job;
use crate::ssh::Runner;

pub fn show_preview(job_id: &str) -> String {
    format!("scontrol show job {job_id}")
}
pub fn hold_preview(job_id: &str) -> String {
    format!("scontrol hold {job_id}")
}
pub fn release_preview(job_id: &str) -> String {
    format!("scontrol release {job_id}")
}
pub fn requeue_preview(job_id: &str) -> String {
    format!("scontrol requeue {job_id}")
}

pub async fn show(runner: &dyn Runner, job_id: &str) -> Result<JobDetails> {
    let out = runner
        .run("scontrol", &["show", "job", job_id])
        .await?
        .check("scontrol show")?;
    parse_scontrol_show_job(&out.stdout)
}

pub async fn hold(runner: &dyn Runner, job_id: &str) -> Result<()> {
    runner
        .run("scontrol", &["hold", job_id])
        .await?
        .check("scontrol hold")?;
    Ok(())
}

pub async fn release(runner: &dyn Runner, job_id: &str) -> Result<()> {
    runner
        .run("scontrol", &["release", job_id])
        .await?
        .check("scontrol release")?;
    Ok(())
}

pub async fn requeue(runner: &dyn Runner, job_id: &str) -> Result<()> {
    runner
        .run("scontrol", &["requeue", job_id])
        .await?
        .check("scontrol requeue")?;
    Ok(())
}
