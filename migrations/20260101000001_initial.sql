-- Initial schema for slurmdash local database.
-- Phase 1.11: Tables for clusters, settings, job snapshots, resource
-- snapshots, command audit log, and cache. LLM and completed-job tables
-- come in later phases.

CREATE TABLE IF NOT EXISTS clusters (
    id          INTEGER PRIMARY KEY,
    name        TEXT NOT NULL UNIQUE,
    host        TEXT,
    port        INTEGER DEFAULT 22,
    username    TEXT,
    ssh_key     TEXT,
    is_local    INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS settings (
    key         TEXT PRIMARY KEY,
    value_json  TEXT NOT NULL,
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS cache (
    key         TEXT PRIMARY KEY,
    value       TEXT NOT NULL,
    expires_at  TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_cache_expires ON cache(expires_at);

CREATE TABLE IF NOT EXISTS job_snapshots (
    id                  INTEGER PRIMARY KEY,
    cluster_id          INTEGER NOT NULL REFERENCES clusters(id) ON DELETE CASCADE,
    job_id              TEXT NOT NULL,
    array_id            TEXT,
    job_name            TEXT,
    username            TEXT,
    account             TEXT,
    partition_name      TEXT,
    state               TEXT,
    reason              TEXT,
    node_list           TEXT,
    cpus                INTEGER,
    gpus                INTEGER,
    memory_mb           INTEGER,
    elapsed_seconds     INTEGER,
    time_limit_seconds  INTEGER,
    submit_time         TEXT,
    start_time          TEXT,
    end_time            TEXT,
    captured_at         TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_job_snapshots_captured ON job_snapshots(captured_at);
CREATE INDEX IF NOT EXISTS idx_job_snapshots_job ON job_snapshots(cluster_id, job_id);

CREATE TABLE IF NOT EXISTS resource_snapshots (
    id                    INTEGER PRIMARY KEY,
    cluster_id            INTEGER NOT NULL REFERENCES clusters(id) ON DELETE CASCADE,
    partition_name        TEXT,
    total_nodes           INTEGER,
    idle_nodes            INTEGER,
    mixed_nodes           INTEGER,
    allocated_nodes       INTEGER,
    down_nodes            INTEGER,
    drained_nodes         INTEGER,
    total_cpus            INTEGER,
    allocated_cpus        INTEGER,
    total_gpus            INTEGER,
    allocated_gpus        INTEGER,
    total_memory_mb       INTEGER,
    allocated_memory_mb   INTEGER,
    captured_at           TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_resource_snapshots_captured ON resource_snapshots(captured_at);

CREATE TABLE IF NOT EXISTS command_audit_log (
    id              INTEGER PRIMARY KEY,
    cluster_id      INTEGER REFERENCES clusters(id) ON DELETE SET NULL,
    command_type    TEXT NOT NULL,
    command_preview TEXT NOT NULL,
    job_id          TEXT,
    user_confirmed  INTEGER NOT NULL,
    success         INTEGER NOT NULL,
    error_message   TEXT,
    executed_at     TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_audit_executed ON command_audit_log(executed_at);
