//! Smoke test: open a fresh SQLite DB, run migrations, exercise the
//! snapshot writer and audit log, and verify counts come back.

use slurmdash::config::Config;
use slurmdash::db::{Db, audit, snapshots};
use slurmdash::slurm::model::Job;
use slurmdash::slurm::state::JobState;

#[tokio::test]
async fn db_migrate_snapshot_and_audit_round_trip() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("slurmdash.db");
    let cfg = Config::default();

    let db = Db::open(Some(path.clone()), &cfg).await.unwrap();
    let cluster_id = snapshots::ensure_cluster(&db.pool, "test-cluster", true)
        .await
        .unwrap();

    let jobs = vec![Job {
        job_id: "1001".into(),
        array_id: None,
        partition: "gpu".into(),
        name: "train".into(),
        user: "alice".into(),
        state: JobState::Running,
        elapsed_seconds: Some(120),
        time_limit_seconds: Some(3600),
        nodes: 1,
        reason_or_nodelist: "nid001".into(),
        submit_time: None,
        start_time: None,
    }];
    snapshots::write_jobs(&db.pool, cluster_id, &jobs)
        .await
        .unwrap();

    audit::record(
        &db.pool,
        audit::Entry {
            cluster_id: Some(cluster_id),
            command_type: "cancel",
            command_preview: "scancel 1001",
            job_id: Some("1001"),
            user_confirmed: true,
            success: true,
            error: None,
        },
    )
    .await
    .unwrap();

    let (snap_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM job_snapshots WHERE cluster_id = ?")
            .bind(cluster_id)
            .fetch_one(&db.pool)
            .await
            .unwrap();
    let (audit_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM command_audit_log WHERE job_id = ?")
            .bind("1001")
            .fetch_one(&db.pool)
            .await
            .unwrap();

    assert_eq!(snap_count, 1, "exactly one job_snapshots row written");
    assert_eq!(audit_count, 1, "exactly one audit row written");
}

#[tokio::test]
async fn db_print_status_is_zero_for_empty_db() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("slurmdash.db");
    let cfg = Config::default();

    let db = Db::open(Some(path), &cfg).await.unwrap();
    // print_status writes to stdout; we just verify it doesn't error.
    db.print_status().await.unwrap();
}
