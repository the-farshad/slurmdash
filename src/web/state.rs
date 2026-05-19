use std::collections::VecDeque;
use std::sync::Arc;

use serde::Serialize;
use tokio::sync::RwLock;

use crate::config::Config;
use crate::db::Db;
use crate::slurm::model::{ClusterResources, Job, Partition};
use crate::ssh::RunnerHandle;

/// Background snapshot of cluster state, refreshed on a timer. Handlers
/// serve this directly so the cluster only sees one call per refresh
/// interval regardless of how many browser clients are polling.
#[derive(Debug, Default, Clone, Serialize)]
pub struct Snapshot {
    pub jobs: Vec<Job>,
    pub partitions: Vec<Partition>,
    pub resources: ClusterResources,
    pub last_refresh: Option<chrono::DateTime<chrono::Utc>>,
    pub last_error: Option<String>,
}

/// Single trailing-history sample. Pushed by the background refresh
/// loop after each snapshot completes. Charts in the web UI consume
/// these.
#[derive(Debug, Clone, Serialize)]
pub struct HistoryPoint {
    pub t: chrono::DateTime<chrono::Utc>,
    pub cpu_pct: f64,
    pub gpu_pct: f64,
    pub mem_pct: f64,
    pub nodes_alloc: u32,
    pub nodes_total: u32,
    pub n_running: u32,
    pub n_pending: u32,
    pub n_failed: u32,
    pub n_total: u32,
}

/// How many history samples to retain in memory. At a 5s refresh that
/// is one hour of trend data; at 1s refresh it is 12 minutes. Old
/// samples drop from the front of the ring.
pub const HISTORY_CAPACITY: usize = 720;

pub struct WebState {
    pub handle: Arc<RunnerHandle>,
    pub db: Option<Db>,
    pub config: Config,
    pub token: String,
    pub readonly: bool,
    pub snapshot: RwLock<Snapshot>,
    pub history: RwLock<VecDeque<HistoryPoint>>,
    pub cluster_id: RwLock<Option<i64>>,
}

impl WebState {
    pub fn new(
        handle: Arc<RunnerHandle>,
        db: Option<Db>,
        config: Config,
        token: String,
        readonly: bool,
    ) -> Self {
        Self {
            handle,
            db,
            config,
            token,
            readonly,
            snapshot: RwLock::new(Snapshot::default()),
            history: RwLock::new(VecDeque::with_capacity(HISTORY_CAPACITY)),
            cluster_id: RwLock::new(None),
        }
    }

    /// Upsert the cluster row up front so foreign-key writes in the
    /// background refresher succeed.
    pub async fn bootstrap_cluster_row(&self) {
        if let Some(db) = &self.db {
            if let Ok(id) = crate::db::snapshots::ensure_cluster(
                &db.pool,
                &self.handle.cluster_name,
                self.handle.is_local,
            )
            .await
            {
                *self.cluster_id.write().await = Some(id);
            }
        }
    }
}
