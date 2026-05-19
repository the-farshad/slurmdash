//! Snapshot writers for the `job_snapshots` and `resource_snapshots` tables.

use anyhow::Result;

use crate::slurm::model::Job;

/// Upsert a `clusters` row keyed by name, returning its id. Used as the FK
/// target for snapshots and audit-log entries.
pub async fn ensure_cluster(pool: &sqlx::SqlitePool, name: &str, is_local: bool) -> Result<i64> {
    sqlx::query(
        "INSERT INTO clusters (name, is_local) VALUES (?, ?) \
         ON CONFLICT(name) DO UPDATE SET updated_at = datetime('now')",
    )
    .bind(name)
    .bind(is_local as i64)
    .execute(pool)
    .await?;

    let row: (i64,) = sqlx::query_as("SELECT id FROM clusters WHERE name = ?")
        .bind(name)
        .fetch_one(pool)
        .await?;
    Ok(row.0)
}

/// Append the current `squeue` result to `job_snapshots`. One row per job.
pub async fn write_jobs(pool: &sqlx::SqlitePool, cluster_id: i64, jobs: &[Job]) -> Result<()> {
    if jobs.is_empty() {
        return Ok(());
    }
    let mut tx = pool.begin().await?;
    for j in jobs {
        sqlx::query(
            "INSERT INTO job_snapshots \
             (cluster_id, job_id, array_id, job_name, username, partition_name, state, \
              reason, elapsed_seconds, time_limit_seconds) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(cluster_id)
        .bind(&j.job_id)
        .bind(j.array_id.as_deref())
        .bind(&j.name)
        .bind(&j.user)
        .bind(&j.partition)
        .bind(j.state.short())
        .bind(&j.reason_or_nodelist)
        .bind(j.elapsed_seconds.map(|v| v as i64))
        .bind(j.time_limit_seconds.map(|v| v as i64))
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}
