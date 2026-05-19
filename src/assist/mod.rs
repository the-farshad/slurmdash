//! LLM prompt assistant.
//!
//! Talks to a chat model (local Ollama by default, optional cloud providers)
//! and turns the response into a [`AssistResponse`]: free-text plus zero or
//! more [`ProposedCommand`]s. Each proposed command must be confirmed by the
//! user through the existing confirm modal before it touches the cluster,
//! and every execution is audit-logged the same way manual actions are.

pub mod anthropic;
pub mod ollama;
pub mod provider;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::slurm::model::{Job, JobDetails, Partition};
use crate::slurm::state::JobState;
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct AssistRequest {
    pub prompt: String,
    pub job_context: Option<JobContext>,
    pub cluster_name: String,
    pub jobs_snapshot: Vec<Job>,
    pub partitions: Vec<Partition>,
    /// Optional human-readable history summary (e.g. "train_resnet: 12 runs,
    /// median 2h14m, 1 timeout"), injected verbatim into the system prompt.
    pub history_summary: Option<String>,
}

#[derive(Debug, Clone)]
pub struct JobContext {
    pub job_id: String,
    pub details: Option<JobDetails>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistResponse {
    pub text: String,
    /// Commands the model proposed. Each is rendered in a confirm modal
    /// before execution.
    pub commands: Vec<ProposedCommand>,
    pub provider: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposedCommand {
    pub kind: ProposedKind,
    /// Exact text that will run if the user confirms.
    pub preview: String,
    /// One-line explanation shown next to the preview.
    pub explanation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ProposedKind {
    /// `scancel <job_id>`
    Cancel { job_id: String },
    /// `scontrol hold <job_id>`
    Hold { job_id: String },
    /// `scontrol release <job_id>`
    Release { job_id: String },
    /// `scontrol requeue <job_id>`
    Requeue { job_id: String },
    /// `sbatch` from a generated script. Caller writes the script to disk
    /// and submits it after confirmation.
    Sbatch { script: String },
    /// Free-form shell command. Only allowed for whitelisted Slurm tools
    /// (squeue, sinfo, sacct, scontrol show); never executed automatically.
    Shell { command: String },
}

/// Build the assist response by calling the configured provider.
pub async fn assist(req: AssistRequest, config: &Config) -> Result<AssistResponse> {
    let provider = provider::resolve(config)?;
    provider.complete(&req).await
}

/// Default system prompt seeded with cluster context. Kept short so it
/// fits in the context window of small local models.
pub(crate) fn system_prompt(req: &AssistRequest) -> String {
    let mut s = String::new();
    s.push_str(
        "You are slurmdash's assistant. The user is on an HPC cluster running Slurm. \
         Answer concisely. When proposing actions, output a fenced code block \
         with the exact command (e.g. `scancel 12345`, `scontrol hold 12345`) on a \
         single line, and explain what it does in plain language. Never assume the \
         action has run — the user will confirm it in a modal before execution.\n\n",
    );
    s.push_str(&format!("Cluster: {}\n", req.cluster_name));
    if !req.partitions.is_empty() {
        s.push_str("Partitions:\n");
        for p in &req.partitions {
            s.push_str(&format!(
                "  {} — nodes alloc {}/{}, cpus alloc {}/{}",
                p.name, p.nodes.allocated, p.nodes.total, p.cpus.allocated, p.cpus.total,
            ));
            if let Some(g) = p.gpus_per_node {
                s.push_str(&format!(", gpus/node {g}"));
            }
            s.push('\n');
        }
    }
    if !req.jobs_snapshot.is_empty() {
        push_jobs_summary(&mut s, &req.jobs_snapshot);
    }
    if let Some(ctx) = &req.job_context {
        s.push_str(&format!("Selected job: {}\n", ctx.job_id));
        if let Some(d) = &ctx.details {
            if let Some(state) = &d.state {
                s.push_str(&format!("  state: {state}\n"));
            }
            if let Some(reason) = &d.reason {
                s.push_str(&format!("  reason: {reason}\n"));
            }
            if let Some(workdir) = &d.workdir {
                s.push_str(&format!("  workdir: {workdir}\n"));
            }
        }
    }
    if let Some(history) = &req.history_summary {
        s.push_str("\nLocal history for similar jobs:\n");
        for line in history.lines() {
            s.push_str(&format!("  {line}\n"));
        }
    }
    s
}

/// Append a compact but informative jobs summary to the system prompt:
/// total count, per-state breakdown, top-5 longest-running, top-3
/// pending with reasons, and top-5 users by job count. Keeps the prompt
/// inside small-model context windows by capping each list.
fn push_jobs_summary(s: &mut String, jobs: &[Job]) {
    s.push_str(&format!("\nJobs visible: {} total\n", jobs.len()));

    // Per-state counts. BTreeMap keeps the order stable for the model.
    let mut by_state: BTreeMap<&str, u32> = BTreeMap::new();
    for j in jobs {
        *by_state.entry(j.state.short()).or_insert(0) += 1;
    }
    if !by_state.is_empty() {
        let parts: Vec<String> = by_state.iter().map(|(k, v)| format!("{k}={v}")).collect();
        s.push_str(&format!("  by state: {}\n", parts.join(", ")));
    }

    // Top-5 running, by elapsed.
    let mut running: Vec<&Job> = jobs
        .iter()
        .filter(|j| j.state == JobState::Running)
        .collect();
    running.sort_by_key(|j| std::cmp::Reverse(j.elapsed_seconds.unwrap_or(0)));
    if !running.is_empty() {
        s.push_str("  longest-running:\n");
        for j in running.iter().take(5) {
            let elapsed = j
                .elapsed_seconds
                .map(short_dur)
                .unwrap_or_else(|| "?".into());
            let limit = j
                .time_limit_seconds
                .map(short_dur)
                .unwrap_or_else(|| "?".into());
            let gpu = if j.uses_gpu() {
                format!(", {} gpu", j.gpus())
            } else {
                String::new()
            };
            s.push_str(&format!(
                "    {} {} on {} ({}/{}{}) — {}\n",
                j.job_id, j.user, j.partition, elapsed, limit, gpu, j.name,
            ));
        }
    }

    // Top-3 pending with reasons.
    let pending: Vec<&Job> = jobs
        .iter()
        .filter(|j| j.state == JobState::Pending)
        .collect();
    if !pending.is_empty() {
        s.push_str(&format!("  pending: {}\n", pending.len()));
        for j in pending.iter().take(3) {
            s.push_str(&format!(
                "    {} {} on {} — reason: {}\n",
                j.job_id, j.user, j.partition, j.reason_or_nodelist,
            ));
        }
    }

    // Top-5 users by job count.
    let mut users: BTreeMap<&str, u32> = BTreeMap::new();
    for j in jobs {
        *users.entry(j.user.as_str()).or_insert(0) += 1;
    }
    let mut user_counts: Vec<(&&str, &u32)> = users.iter().collect();
    user_counts.sort_by_key(|(_, c)| std::cmp::Reverse(**c));
    if user_counts.len() > 1 {
        let summary: Vec<String> = user_counts
            .iter()
            .take(5)
            .map(|(u, c)| format!("{u}={c}"))
            .collect();
        s.push_str(&format!("  top users: {}\n", summary.join(", ")));
    }
}

fn short_dur(s: u64) -> String {
    if s < 60 {
        format!("{s}s")
    } else if s < 3600 {
        format!("{}m", s / 60)
    } else if s < 86_400 {
        format!("{}h{}m", s / 3600, (s % 3600) / 60)
    } else {
        format!("{}d", s / 86_400)
    }
}

/// Pull `scancel N` / `scontrol hold N` / `sbatch <<EOF…EOF` style snippets
/// out of the model's free-text reply so we can build proposed commands.
pub(crate) fn extract_commands(text: &str) -> Vec<ProposedCommand> {
    let mut out = Vec::new();
    let re =
        regex::Regex::new(r"(?m)^\s*(scancel|scontrol\s+(?:hold|release|requeue)|sbatch)\b[^\n]*$")
            .unwrap();
    for m in re.find_iter(text) {
        let line = m.as_str().trim();
        let kind = classify(line);
        if let Some(k) = kind {
            let preview = line.to_string();
            out.push(ProposedCommand {
                kind: k,
                preview,
                explanation: String::new(),
            });
        }
    }
    out
}

fn classify(line: &str) -> Option<ProposedKind> {
    let mut it = line.split_whitespace();
    let head = it.next()?;
    match head {
        "scancel" => {
            let id = it.next()?.to_string();
            Some(ProposedKind::Cancel { job_id: id })
        }
        "scontrol" => {
            let sub = it.next()?;
            let id = it.next()?.to_string();
            match sub {
                "hold" => Some(ProposedKind::Hold { job_id: id }),
                "release" => Some(ProposedKind::Release { job_id: id }),
                "requeue" => Some(ProposedKind::Requeue { job_id: id }),
                _ => None,
            }
        }
        "sbatch" => Some(ProposedKind::Sbatch {
            script: line.to_string(),
        }),
        _ => None,
    }
}
