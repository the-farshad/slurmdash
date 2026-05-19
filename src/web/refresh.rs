//! Background refresher: fills `WebState.snapshot` on a timer so all browser
//! clients share the same recent data without each pollerhammering the
//! cluster.

use std::sync::Arc;
use std::time::Duration;

use crate::db::snapshots;
use crate::slurm::model::ClusterResources;
use crate::slurm::{sinfo, squeue};
use crate::web::state::{Snapshot, WebState};

pub async fn run(state: Arc<WebState>, interval: Duration) {
    let mut ticker = tokio::time::interval(interval);
    loop {
        ticker.tick().await;
        let snapshot = build_snapshot(&state).await;
        *state.snapshot.write().await = snapshot;
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
