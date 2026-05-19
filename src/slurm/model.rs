use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::state::JobState;

/// Counts of nodes or CPUs in each Slurm aggregation bucket. The four-tuple
/// `Allocated/Idle/Other/Total` (AIOT) is the format Slurm itself uses in
/// `sinfo --format=%F` / `%C`.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Aiot {
    pub allocated: u32,
    pub idle: u32,
    pub other: u32,
    pub total: u32,
}

impl Aiot {
    pub fn pct_allocated(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.allocated as f64 / self.total as f64
        }
    }
}

/// One Slurm partition. Populated from `sinfo` output. Memory and GPU
/// counts are per node (Slurm reports them that way); multiply by
/// `nodes.total` for cluster totals.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Partition {
    pub name: String,
    pub default: bool,
    pub nodes: Aiot,
    pub cpus: Aiot,
    pub memory_mb_per_node: Option<u64>,
    pub gpus_per_node: Option<u32>,
    pub gpu_type: Option<String>,
}

impl Partition {
    pub fn total_memory_mb(&self) -> Option<u64> {
        self.memory_mb_per_node.map(|m| m * self.nodes.total as u64)
    }

    pub fn allocated_memory_mb(&self) -> Option<u64> {
        self.memory_mb_per_node
            .map(|m| m * self.nodes.allocated as u64)
    }

    pub fn total_gpus(&self) -> Option<u32> {
        self.gpus_per_node.map(|g| g * self.nodes.total)
    }

    pub fn allocated_gpus(&self) -> Option<u32> {
        self.gpus_per_node.map(|g| g * self.nodes.allocated)
    }
}

/// Cluster-wide totals computed from a slice of [`Partition`]s.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct ClusterResources {
    pub nodes: Aiot,
    pub cpus: Aiot,
    pub memory_mb: AiotU64,
    pub gpus: Aiot,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct AiotU64 {
    pub allocated: u64,
    pub idle: u64,
    pub other: u64,
    pub total: u64,
}

impl AiotU64 {
    pub fn pct_allocated(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.allocated as f64 / self.total as f64
        }
    }
}

impl ClusterResources {
    pub fn from_partitions(parts: &[Partition]) -> Self {
        let mut out = Self::default();
        for p in parts {
            out.nodes.allocated += p.nodes.allocated;
            out.nodes.idle += p.nodes.idle;
            out.nodes.other += p.nodes.other;
            out.nodes.total += p.nodes.total;

            out.cpus.allocated += p.cpus.allocated;
            out.cpus.idle += p.cpus.idle;
            out.cpus.other += p.cpus.other;
            out.cpus.total += p.cpus.total;

            if let Some(mem_per) = p.memory_mb_per_node {
                out.memory_mb.allocated += mem_per * p.nodes.allocated as u64;
                out.memory_mb.idle += mem_per * p.nodes.idle as u64;
                out.memory_mb.other += mem_per * p.nodes.other as u64;
                out.memory_mb.total += mem_per * p.nodes.total as u64;
            }

            if let Some(g) = p.gpus_per_node {
                out.gpus.allocated += g * p.nodes.allocated;
                out.gpus.idle += g * p.nodes.idle;
                out.gpus.other += g * p.nodes.other;
                out.gpus.total += g * p.nodes.total;
            }
        }
        out
    }
}

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
