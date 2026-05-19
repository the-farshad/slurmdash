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

pub struct WebState {
    pub handle: Arc<RunnerHandle>,
    pub db: Option<Db>,
    pub config: Config,
    pub token: String,
    pub readonly: bool,
    pub snapshot: RwLock<Snapshot>,
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
