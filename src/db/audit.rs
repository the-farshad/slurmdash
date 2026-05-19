//! Append-only audit log for destructive Slurm commands.

use anyhow::Result;

#[derive(Debug, Clone)]
pub struct Entry<'a> {
    pub cluster_id: Option<i64>,
    pub command_type: &'a str,
    pub command_preview: &'a str,
    pub job_id: Option<&'a str>,
    pub user_confirmed: bool,
    pub success: bool,
    pub error: Option<&'a str>,
}

pub async fn record(pool: &sqlx::SqlitePool, entry: Entry<'_>) -> Result<()> {
    sqlx::query(
        "INSERT INTO command_audit_log \
         (cluster_id, command_type, command_preview, job_id, user_confirmed, success, error_message) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(entry.cluster_id)
    .bind(entry.command_type)
    .bind(entry.command_preview)
    .bind(entry.job_id)
    .bind(entry.user_confirmed as i64)
    .bind(entry.success as i64)
    .bind(entry.error)
    .execute(pool)
    .await?;
    Ok(())
}
