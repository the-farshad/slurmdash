use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::state::JobState;

/// Compact row-level view of a job, populated from a single `squeue` call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub job_id: String,
    pub array_id: Option<String>,
    pub partition: String,
    pub name: String,
    pub user: String,
    pub state: JobState,
    pub elapsed_seconds: Option<u64>,
    pub time_limit_seconds: Option<u64>,
    pub nodes: u32,
    pub reason_or_nodelist: String,
}

/// Detailed view of a single job, populated from `scontrol show job`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct JobDetails {
    pub job_id: String,
    pub job_name: Option<String>,
    pub user: Option<String>,
    pub account: Option<String>,
    pub partition: Option<String>,
    pub qos: Option<String>,
    pub state: Option<String>,
    pub reason: Option<String>,
    pub command: Option<String>,
    pub workdir: Option<String>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub stdin: Option<String>,
    pub priority: Option<String>,
    pub dependency: Option<String>,
    pub submit_time: Option<DateTime<Utc>>,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub exit_code: Option<String>,
    pub nodes_alloc: Option<String>,
    pub num_nodes: Option<u32>,
    pub num_cpus: Option<u32>,
    pub raw: Vec<(String, String)>,
}
