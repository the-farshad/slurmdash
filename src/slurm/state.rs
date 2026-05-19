use serde::{Deserialize, Serialize};
use std::fmt;

/// Slurm job state, as reported by squeue/sacct.
///
/// Names follow Slurm's own short codes (R, PD, …) and full names
/// (RUNNING, PENDING, …). Unknown values are kept as a string under
/// [`JobState::Other`] so we never silently lose data.
#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum JobState {
    Pending,
    Running,
    Suspended,
    Completing,
    Completed,
    Cancelled,
    Failed,
    Timeout,
    NodeFail,
    Preempted,
    BootFail,
    Deadline,
    OutOfMemory,
    Held,
    Other(String),
}

impl JobState {
    pub fn parse(s: &str) -> Self {
        match s.trim() {
            "R" | "RUNNING" => Self::Running,
            "PD" | "PENDING" => Self::Pending,
            "S" | "SUSPENDED" => Self::Suspended,
            "CG" | "COMPLETING" => Self::Completing,
            "CD" | "COMPLETED" => Self::Completed,
            "CA" | "CANCELLED" | "CANCELED" => Self::Cancelled,
            "F" | "FAILED" => Self::Failed,
            "TO" | "TIMEOUT" => Self::Timeout,
            "NF" | "NODE_FAIL" => Self::NodeFail,
            "PR" | "PREEMPTED" => Self::Preempted,
            "BF" | "BOOT_FAIL" => Self::BootFail,
            "DL" | "DEADLINE" => Self::Deadline,
            "OOM" | "OUT_OF_MEMORY" => Self::OutOfMemory,
            "H" | "HELD" => Self::Held,
            other => Self::Other(other.to_string()),
        }
    }

    pub fn short(&self) -> &str {
        match self {
            Self::Running => "R",
            Self::Pending => "PD",
            Self::Suspended => "S",
            Self::Completing => "CG",
            Self::Completed => "CD",
            Self::Cancelled => "CA",
            Self::Failed => "F",
            Self::Timeout => "TO",
            Self::NodeFail => "NF",
            Self::Preempted => "PR",
            Self::BootFail => "BF",
            Self::Deadline => "DL",
            Self::OutOfMemory => "OOM",
            Self::Held => "H",
            Self::Other(s) => s,
        }
    }
}

impl fmt::Display for JobState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.short())
    }
}
