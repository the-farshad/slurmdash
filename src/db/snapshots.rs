//! Snapshot writers for the `job_snapshots` and `resource_snapshots` tables.

use anyhow::Result;

use crate::slurm::model::{Job, Partition};

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

/// Append the current `sinfo` result to `resource_snapshots`. One row per partition.
pub async fn write_resources(
    pool: &sqlx::SqlitePool,
    cluster_id: i64,
    partitions: &[Partition],
) -> Result<()> {
    if partitions.is_empty() {
        return Ok(());
    }
    let mut tx = pool.begin().await?;
    for p in partitions {
        sqlx::query(
            "INSERT INTO resource_snapshots \
             (cluster_id, partition_name, total_nodes, idle_nodes, mixed_nodes, \
              allocated_nodes, down_nodes, drained_nodes, total_cpus, allocated_cpus, \
              total_gpus, allocated_gpus, total_memory_mb, allocated_memory_mb) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(cluster_id)
        .bind(&p.name)
        .bind(p.nodes.total as i64)
        .bind(p.nodes.idle as i64)
        // `mixed` / `down` / `drained` aren't separately reported by our
        // current sinfo format; lump them under `other` and store as NULL.
        .bind::<Option<i64>>(None)
        .bind(p.nodes.allocated as i64)
        .bind::<Option<i64>>(None)
        .bind::<Option<i64>>(None)
        .bind(p.cpus.total as i64)
        .bind(p.cpus.allocated as i64)
        .bind(p.total_gpus().map(|v| v as i64))
        .bind(p.allocated_gpus().map(|v| v as i64))
        .bind(p.total_memory_mb().map(|v| v as i64))
        .bind(p.allocated_memory_mb().map(|v| v as i64))
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
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
              reason, elapsed_seconds, time_limit_seconds, submit_time, start_time) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
        .bind(j.submit_time.map(|t| t.to_rfc3339()))
        .bind(j.start_time.map(|t| t.to_rfc3339()))
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}
