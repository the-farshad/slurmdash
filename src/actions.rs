//! Slurm action dispatcher with audit-log recording.
//!
//! Used by both the CLI subcommands and the TUI confirm-modal flow. Every
//! call:
//! 1. Renders an exact command preview (for the confirm modal / logs).
//! 2. Records a row in `command_audit_log` before and after execution.
//! 3. Returns the command's result so callers can surface errors.

use anyhow::Result;

use crate::db::{Db, audit, snapshots};
use crate::slurm::{scancel, scontrol};
use crate::ssh::Runner;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ActionKind {
    Cancel,
    Hold,
    Release,
    Requeue,
}

impl ActionKind {
    pub fn label(self) -> &'static str {
        match self {
            ActionKind::Cancel => "cancel",
            ActionKind::Hold => "hold",
            ActionKind::Release => "release",
            ActionKind::Requeue => "requeue",
        }
    }

    pub fn preview(self, job_id: &str) -> String {
        match self {
            ActionKind::Cancel => scancel::preview(job_id),
            ActionKind::Hold => scontrol::hold_preview(job_id),
            ActionKind::Release => scontrol::release_preview(job_id),
            ActionKind::Requeue => scontrol::requeue_preview(job_id),
        }
    }
}

/// Run a destructive action and write an audit-log entry. The
/// `user_confirmed` flag should be `true` for TUI actions and CLI
/// subcommands (which are interactive by nature) and `false` if/when we add
/// a future dry-run mode.
pub async fn run(
    kind: ActionKind,
    job_id: &str,
    runner: &dyn Runner,
    db: Option<&Db>,
    cluster_name: &str,
    is_local: bool,
    user_confirmed: bool,
) -> Result<()> {
    let preview = kind.preview(job_id);

    let cluster_id = match db {
        Some(d) => snapshots::ensure_cluster(&d.pool, cluster_name, is_local)
            .await
            .ok(),
        None => None,
    };

    tracing::info!(
        cluster = %cluster_name,
        action = kind.label(),
        job_id = job_id,
        preview = %preview,
        confirmed = user_confirmed,
        "destructive action start"
    );
    let start = std::time::Instant::now();
    let result = match kind {
        ActionKind::Cancel => scancel::cancel(runner, job_id).await,
        ActionKind::Hold => scontrol::hold(runner, job_id).await,
        ActionKind::Release => scontrol::release(runner, job_id).await,
        ActionKind::Requeue => scontrol::requeue(runner, job_id).await,
    };
    match &result {
        Ok(()) => tracing::info!(
            cluster = %cluster_name,
            action = kind.label(),
            job_id = job_id,
            elapsed_ms = start.elapsed().as_millis() as u64,
            "destructive action ok"
        ),
        Err(e) => tracing::warn!(
            cluster = %cluster_name,
            action = kind.label(),
            job_id = job_id,
            error = %e,
            "destructive action failed"
        ),
    }

    if let Some(d) = db {
        let err_string = result.as_ref().err().map(|e| format!("{e}"));
        let _ = audit::record(
            &d.pool,
            audit::Entry {
                cluster_id,
                command_type: kind.label(),
                command_preview: &preview,
                job_id: Some(job_id),
                user_confirmed,
                success: result.is_ok(),
                error: err_string.as_deref(),
            },
        )
        .await;
    }

    result
}
