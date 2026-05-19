//! History-driven recommendations.
//!
//! Phase 5 works against the `job_snapshots` + `resource_snapshots` tables
//! that Phase 1.12 and 2.3 have been filling. A sacct-driven `completed_jobs`
//! mirror is the right long-term source (it carries MaxRSS, exit codes, etc.)
//! but is not yet implemented — these analyzers stay deterministic and
//! conservative against what we have so they degrade gracefully on a
//! fresh database.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct JobNameStats {
    pub job_name: String,
    pub runs: u32,
    pub elapsed_min_seconds: Option<u64>,
    pub elapsed_max_seconds: Option<u64>,
    pub elapsed_p50_seconds: Option<u64>,
    pub wait_min_seconds: Option<u64>,
    pub wait_max_seconds: Option<u64>,
    pub wait_p50_seconds: Option<u64>,
    pub failures: u32,
    pub timeouts: u32,
    pub cancellations: u32,
    pub completions: u32,
    pub last_seen: Option<chrono::DateTime<chrono::Utc>>,
    /// Recent runs (up to 12), most recent first.
    pub recent: Vec<RecentRun>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RecentRun {
    pub job_id: String,
    pub state: String,
    pub elapsed_seconds: Option<u64>,
    pub wait_seconds: Option<u64>,
    pub captured_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PartitionPressure {
    pub partition: String,
    pub avg_pending: f64,
    pub avg_running: f64,
    pub samples: u32,
}

/// Roll up walltime + wait + terminal-state stats per job name for a
/// cluster over the trailing `since_days` days. Also collects up to 12
/// recent runs per name for the details-panel chip strip.
type JobNameRow = (
    String,         // job_name
    String,         // job_id
    Option<i64>,    // elapsed_seconds
    Option<String>, // submit_time
    Option<String>, // start_time
    String,         // state
    String,         // captured_at
);

pub async fn job_name_stats(
    pool: &SqlitePool,
    cluster_id: i64,
    since_days: i64,
) -> Result<Vec<JobNameStats>> {
    let rows: Vec<JobNameRow> = sqlx::query_as(
        r#"
        SELECT
            job_name,
            job_id,
            elapsed_seconds,
            submit_time,
            start_time,
            state,
            MAX(captured_at) AS captured_at
        FROM job_snapshots
        WHERE cluster_id = ?
          AND job_name IS NOT NULL
          AND captured_at >= datetime('now', ?)
        GROUP BY job_id, job_name
        "#,
    )
    .bind(cluster_id)
    .bind(format!("-{} days", since_days))
    .fetch_all(pool)
    .await?;

    let mut by_name: std::collections::BTreeMap<String, JobNameStats> =
        std::collections::BTreeMap::new();
    let mut elapsed_buckets: std::collections::BTreeMap<String, Vec<u64>> =
        std::collections::BTreeMap::new();
    let mut wait_buckets: std::collections::BTreeMap<String, Vec<u64>> =
        std::collections::BTreeMap::new();
    let mut recent_buckets: std::collections::BTreeMap<String, Vec<RecentRun>> =
        std::collections::BTreeMap::new();

    for (name, job_id, elapsed, submit_time, start_time, state, captured_at) in rows {
        let entry = by_name.entry(name.clone()).or_insert_with(|| JobNameStats {
            job_name: name.clone(),
            ..Default::default()
        });
        entry.runs += 1;
        match state.as_str() {
            "F" | "FAILED" => entry.failures += 1,
            "TO" | "TIMEOUT" => entry.timeouts += 1,
            "CA" | "CANCELLED" => entry.cancellations += 1,
            "CD" | "COMPLETED" => entry.completions += 1,
            _ => {}
        }
        let captured_dt = chrono::DateTime::parse_from_rfc3339(&captured_at)
            .ok()
            .map(|t| t.with_timezone(&chrono::Utc));
        if let Some(seen) = captured_dt {
            entry.last_seen = Some(match entry.last_seen {
                Some(prev) if prev > seen => prev,
                _ => seen,
            });
        }
        let elapsed_u = elapsed.and_then(|e| if e > 0 { Some(e as u64) } else { None });
        if let Some(e) = elapsed_u {
            elapsed_buckets.entry(name.clone()).or_default().push(e);
        }
        let wait_u = match (
            submit_time.as_deref().and_then(parse_dt),
            start_time.as_deref().and_then(parse_dt),
        ) {
            (Some(s), Some(t)) if t >= s => Some((t - s).num_seconds() as u64),
            _ => None,
        };
        if let Some(w) = wait_u {
            wait_buckets.entry(name.clone()).or_default().push(w);
        }
        recent_buckets.entry(name).or_default().push(RecentRun {
            job_id,
            state,
            elapsed_seconds: elapsed_u,
            wait_seconds: wait_u,
            captured_at: captured_dt,
        });
    }

    fn fill_percentiles(target: &mut JobNameStats, samples: &mut [u64]) {
        if samples.is_empty() {
            return;
        }
        samples.sort_unstable();
        target.elapsed_min_seconds = samples.first().copied();
        target.elapsed_max_seconds = samples.last().copied();
        let mid = samples.len() / 2;
        target.elapsed_p50_seconds = Some(samples[mid]);
    }

    for (name, mut samples) in elapsed_buckets {
        if let Some(entry) = by_name.get_mut(&name) {
            fill_percentiles(entry, &mut samples);
        }
    }

    for (name, mut samples) in wait_buckets {
        if let Some(entry) = by_name.get_mut(&name) {
            samples.sort_unstable();
            entry.wait_min_seconds = samples.first().copied();
            entry.wait_max_seconds = samples.last().copied();
            let mid = samples.len() / 2;
            entry.wait_p50_seconds = Some(samples[mid]);
        }
    }

    for (name, mut runs) in recent_buckets {
        if let Some(entry) = by_name.get_mut(&name) {
            runs.sort_by_key(|r| std::cmp::Reverse(r.captured_at));
            runs.truncate(12);
            entry.recent = runs;
        }
    }

    let mut out: Vec<JobNameStats> = by_name.into_values().collect();
    out.sort_by_key(|s| std::cmp::Reverse(s.runs));
    Ok(out)
}

fn parse_dt(s: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|t| t.with_timezone(&chrono::Utc))
}

/// Stats for a specific job name (or empty when no rows match).
pub async fn job_name(
    pool: &SqlitePool,
    cluster_id: i64,
    name: &str,
    since_days: i64,
) -> Result<Option<JobNameStats>> {
    let all = job_name_stats(pool, cluster_id, since_days).await?;
    Ok(all.into_iter().find(|s| s.job_name == name))
}

/// Average pending/running job count per partition over the trailing window.
pub async fn partition_pressure(
    pool: &SqlitePool,
    cluster_id: i64,
    since_days: i64,
) -> Result<Vec<PartitionPressure>> {
    // Snapshot-based: for each refresh tick, count PD vs R per partition.
    // Then average across all tick timestamps.
    let rows: Vec<(String, String, String)> = sqlx::query_as(
        r#"
        SELECT partition_name, state, captured_at
        FROM job_snapshots
        WHERE cluster_id = ?
          AND partition_name IS NOT NULL
          AND captured_at >= datetime('now', ?)
        "#,
    )
    .bind(cluster_id)
    .bind(format!("-{} days", since_days))
    .fetch_all(pool)
    .await?;

    // Bucket by (captured_at, partition) → counts.
    let mut tick_counts: std::collections::BTreeMap<(String, String), (u32, u32)> =
        std::collections::BTreeMap::new();
    for (partition, state, captured_at) in rows {
        let key = (captured_at, partition);
        let entry = tick_counts.entry(key).or_insert((0, 0));
        match state.as_str() {
            "PD" | "PENDING" => entry.0 += 1,
            "R" | "RUNNING" => entry.1 += 1,
            _ => {}
        }
    }

    // Roll up by partition.
    let mut by_part: std::collections::BTreeMap<String, (u64, u64, u32)> =
        std::collections::BTreeMap::new();
    for ((_, part), (pd, r)) in tick_counts {
        let e = by_part.entry(part).or_insert((0, 0, 0));
        e.0 += pd as u64;
        e.1 += r as u64;
        e.2 += 1;
    }

    let mut out: Vec<PartitionPressure> = by_part
        .into_iter()
        .map(|(partition, (pd, r, s))| PartitionPressure {
            partition,
            avg_pending: if s == 0 { 0.0 } else { pd as f64 / s as f64 },
            avg_running: if s == 0 { 0.0 } else { r as f64 / s as f64 },
            samples: s,
        })
        .collect();
    out.sort_by(|a, b| {
        a.avg_pending
            .partial_cmp(&b.avg_pending)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(out)
}

/// Format a `JobNameStats` into a one-paragraph human summary used in CLI
/// output and the assist system prompt.
pub fn summarize(stats: &JobNameStats) -> String {
    let mut s = format!("{}: {} runs", stats.job_name, stats.runs);
    if let (Some(p50), Some(max)) = (stats.elapsed_p50_seconds, stats.elapsed_max_seconds) {
        s.push_str(&format!(
            ", median elapsed {}, max {}",
            humanize_dur(p50),
            humanize_dur(max)
        ));
    }
    let bad = stats.failures + stats.timeouts;
    if bad > 0 {
        s.push_str(&format!(
            ", {bad} failed/timed out ({} F / {} TO)",
            stats.failures, stats.timeouts
        ));
    }
    if stats.cancellations > 0 {
        s.push_str(&format!(", {} cancelled", stats.cancellations));
    }
    s
}

fn humanize_dur(seconds: u64) -> String {
    let h = seconds / 3600;
    let m = (seconds % 3600) / 60;
    if h > 0 {
        format!("{h}h{m:02}m")
    } else {
        format!("{m}m")
    }
}
