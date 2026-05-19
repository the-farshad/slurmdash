//! Background refresher: fills `WebState.snapshot` on a timer so all browser
//! clients share the same recent data without each pollerhammering the
//! cluster.

use std::sync::Arc;
use std::time::Duration;

use crate::db::snapshots;
use crate::slurm::model::ClusterResources;
use crate::slurm::state::JobState;
use crate::slurm::{sinfo, squeue};
use crate::web::state::{HISTORY_CAPACITY, HistoryPoint, Snapshot, WebState};

pub async fn run(state: Arc<WebState>, interval: Duration) {
    let mut ticker = tokio::time::interval(interval);
    loop {
        ticker.tick().await;
        let snapshot = build_snapshot(&state).await;
        let point = derive_history_point(&snapshot);
        *state.snapshot.write().await = snapshot;
        let mut hist = state.history.write().await;
        if hist.len() >= HISTORY_CAPACITY {
            hist.pop_front();
        }
        hist.push_back(point);
    }
}

fn derive_history_point(s: &Snapshot) -> HistoryPoint {
    let mut n_running = 0u32;
    let mut n_pending = 0u32;
    let mut n_failed = 0u32;
    for j in &s.jobs {
        match j.state {
            JobState::Running => n_running += 1,
            JobState::Pending => n_pending += 1,
            JobState::Failed
            | JobState::Timeout
            | JobState::NodeFail
            | JobState::BootFail
            | JobState::Deadline
            | JobState::OutOfMemory => n_failed += 1,
            _ => {}
        }
    }
    HistoryPoint {
        t: s.last_refresh.unwrap_or_else(chrono::Utc::now),
        cpu_pct: s.resources.cpus.pct_allocated() * 100.0,
        gpu_pct: s.resources.gpus.pct_allocated() * 100.0,
        mem_pct: s.resources.memory_mb.pct_allocated() * 100.0,
        nodes_alloc: s.resources.nodes.allocated,
        nodes_total: s.resources.nodes.total,
        n_running,
        n_pending,
        n_failed,
        n_total: s.jobs.len() as u32,
    }
}

async fn build_snapshot(state: &WebState) -> Snapshot {
    let runner = state.handle.runner.as_ref();
    let mut snap = Snapshot::default();

    let opts = squeue::Options {
        me: false,
        ..Default::default()
    };
    match squeue::list(runner, &opts).await {
        Ok(jobs) => {
            if let (Some(db), Some(cid)) = (&state.db, *state.cluster_id.read().await) {
                let _ = snapshots::write_jobs(&db.pool, cid, &jobs).await;
            }
            snap.jobs = jobs;
        }
        Err(e) => {
            snap.last_error = Some(format!("squeue: {e}"));
        }
    }

    match sinfo::list_partitions(runner).await {
        Ok(parts) => {
            snap.resources = ClusterResources::from_partitions(&parts);
            if let (Some(db), Some(cid)) = (&state.db, *state.cluster_id.read().await) {
                let _ = snapshots::write_resources(&db.pool, cid, &parts).await;
            }
            snap.partitions = parts;
        }
        Err(e) => {
            if snap.last_error.is_none() {
                snap.last_error = Some(format!("sinfo: {e}"));
            }
        }
    }

    snap.last_refresh = Some(chrono::Utc::now());
    snap
}
