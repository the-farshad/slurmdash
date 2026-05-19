use anyhow::Result;
use regex::Regex;
use std::collections::BTreeMap;
use std::sync::OnceLock;

use super::model::{Aiot, Job, JobDetails, Partition};
use super::state::JobState;

/// Format string passed to `squeue --format=`. Order must match
/// [`parse_squeue_text`]. Fields:
/// JobID | Partition | Name | User | State | Time | TimeLimit | Nodes |
/// Reason | SubmitTime | StartTime | Gres | MinMemory
pub const SQUEUE_FORMAT: &str = "%i|%P|%j|%u|%T|%M|%l|%D|%R|%V|%S|%b|%m";

fn parse_slurm_datetime(s: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    let s = s.trim();
    if s.is_empty() || s == "N/A" || s == "Unknown" || s == "None" {
        return None;
    }
    chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S")
        .ok()
        .map(|n| n.and_utc())
}

/// Parse `squeue --noheader --format=...` text output.
///
/// Fields (in order): JobID|Partition|Name|User|State|Time|TimeLimit|Nodes|Reason
pub fn parse_squeue_text(s: &str) -> Vec<Job> {
    let mut jobs = Vec::new();
    for line in s.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut it = line.split('|');
        let raw_id = it.next().unwrap_or("").to_string();
        let partition = it.next().unwrap_or("").to_string();
        let name = it.next().unwrap_or("").to_string();
        let user = it.next().unwrap_or("").to_string();
        let state = JobState::parse(it.next().unwrap_or(""));
        let elapsed = parse_duration(it.next().unwrap_or(""));
        let time_limit = parse_duration(it.next().unwrap_or(""));
        let nodes = it.next().unwrap_or("0").parse::<u32>().unwrap_or(0);
        let reason_or_nodelist = it.next().unwrap_or("").to_string();
        let submit_time = parse_slurm_datetime(it.next().unwrap_or(""));
        let start_time = parse_slurm_datetime(it.next().unwrap_or(""));
        let gres = it.next().unwrap_or("").trim().to_string();
        let min_mem_mb = parse_slurm_mem(it.next().unwrap_or(""));

        let (job_id, array_id) = split_array_id(&raw_id);

        jobs.push(Job {
            job_id,
            array_id,
            partition,
            name,
            user,
            state,
            elapsed_seconds: elapsed,
            time_limit_seconds: time_limit,
            nodes,
            reason_or_nodelist,
            submit_time,
            start_time,
            gres: if gres == "(null)" || gres == "N/A" {
                String::new()
            } else {
                gres
            },
            min_mem_mb,
        });
    }
    jobs
}

/// Parse the multi-section `scontrol show job <id>` output.
///
/// Each section is a flat list of `Key=Value` pairs, sometimes wrapped over
/// multiple lines and separated by whitespace.
pub fn parse_scontrol_show_job(s: &str) -> Result<JobDetails> {
    let mut details = JobDetails::default();
    for (k, v) in kv_pairs(s) {
        match k.as_str() {
            "JobId" => details.job_id = v.clone(),
            "JobName" => details.job_name = Some(v.clone()),
            "UserId" => details.user = Some(v.clone()),
            "Account" => details.account = Some(v.clone()),
            "Partition" => details.partition = Some(v.clone()),
            "QOS" => details.qos = Some(v.clone()),
            "JobState" => details.state = Some(v.clone()),
            "Reason" => details.reason = Some(v.clone()),
            "Command" => details.command = Some(v.clone()),
            "WorkDir" => details.workdir = Some(v.clone()),
            "StdOut" => details.stdout = Some(v.clone()),
            "StdErr" => details.stderr = Some(v.clone()),
            "StdIn" => details.stdin = Some(v.clone()),
            "Priority" => details.priority = Some(v.clone()),
            "Dependency" => details.dependency = Some(v.clone()),
            "NumNodes" => details.num_nodes = v.split('-').next().and_then(|x| x.parse().ok()),
            "NumCPUs" => details.num_cpus = v.parse().ok(),
            "NodeList" => details.nodes_alloc = Some(v.clone()),
            "ExitCode" => details.exit_code = Some(v.clone()),
            _ => {}
        }
        details.raw.push((k, v));
    }
    Ok(details)
}

/// Walk a `scontrol show job` blob and yield `(key, value)` pairs in order.
fn kv_pairs(s: &str) -> Vec<(String, String)> {
    // scontrol prints whitespace-separated Key=Value tokens, where the value
    // can contain spaces if (and only if) it's the last token on its line.
    // Be conservative: split on whitespace, but join trailing-line tokens
    // when a token doesn't contain '='.
    let mut out = Vec::new();
    for line in s.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let tokens: Vec<&str> = trimmed.split_whitespace().collect();
        for (i, tok) in tokens.iter().enumerate() {
            if let Some(eq) = tok.find('=') {
                let key = tok[..eq].to_string();
                let mut value = tok[eq + 1..].to_string();
                // If this is the last token on the line, gobble any trailing
                // tokens that lack '=' (rare in scontrol output but safe).
                if i + 1 == tokens.len() {
                    // no-op; already captured up to end
                }
                // scontrol uses '(null)' for missing fields; normalize.
                if value == "(null)" {
                    value.clear();
                }
                out.push((key, value));
            }
        }
    }
    out
}

/// "12345_7" → ("12345_7", Some("12345")). Plain ids → (id, None).
fn split_array_id(raw: &str) -> (String, Option<String>) {
    if let Some(idx) = raw.find('_') {
        let base = &raw[..idx];
        (raw.to_string(), Some(base.to_string()))
    } else {
        (raw.to_string(), None)
    }
}

fn slurm_duration_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        // Matches forms like: "1-02:03:04", "02:03:04", "03:04", "5"
        Regex::new(r"^(?:(\d+)-)?(?:(\d+):)?(\d+):(\d+)$|^(\d+)$").unwrap()
    })
}

/// Parse `sinfo --noheader --format="%P|%F|%C|%m|%G"` text output.
///
/// Slurm sometimes emits the same partition on multiple rows (one per node
/// state); we sum into a single [`Partition`] entry keyed by name.
pub fn parse_sinfo_text(s: &str) -> Vec<Partition> {
    let mut by_name: BTreeMap<String, Partition> = BTreeMap::new();
    for line in s.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut it = line.split('|');
        let raw_name = it.next().unwrap_or("");
        let is_default = raw_name.ends_with('*');
        let name = raw_name.trim_end_matches('*').trim().to_string();
        if name.is_empty() {
            continue;
        }
        let nodes = parse_aiot(it.next().unwrap_or(""));
        let cpus = parse_aiot(it.next().unwrap_or(""));
        let mem_token = it.next().unwrap_or("").trim();
        let memory: Option<u64> = if mem_token.is_empty() || mem_token == "N/A" {
            None
        } else {
            mem_token.parse().ok()
        };
        let gres = it.next().unwrap_or("").trim().to_string();
        let (gpus_per_node, gpu_type) = parse_gres(&gres);

        let entry = by_name.entry(name.clone()).or_insert_with(|| Partition {
            name: name.clone(),
            default: is_default,
            ..Default::default()
        });
        // Sum across state-rows; keep the first non-None memory/gres values.
        entry.default = entry.default || is_default;
        entry.nodes.allocated += nodes.allocated;
        entry.nodes.idle += nodes.idle;
        entry.nodes.other += nodes.other;
        entry.nodes.total += nodes.total;
        entry.cpus.allocated += cpus.allocated;
        entry.cpus.idle += cpus.idle;
        entry.cpus.other += cpus.other;
        entry.cpus.total += cpus.total;
        if entry.memory_mb_per_node.is_none() {
            entry.memory_mb_per_node = memory;
        }
        if entry.gpus_per_node.is_none() {
            entry.gpus_per_node = gpus_per_node;
        }
        if entry.gpu_type.is_none() {
            entry.gpu_type = gpu_type;
        }
    }
    by_name.into_values().collect()
}

/// Parse a Slurm `Allocated/Idle/Other/Total` aggregate (e.g. "12/4/0/16").
fn parse_aiot(s: &str) -> Aiot {
    let mut it = s.trim().split('/');
    Aiot {
        allocated: it.next().and_then(|x| x.parse().ok()).unwrap_or(0),
        idle: it.next().and_then(|x| x.parse().ok()).unwrap_or(0),
        other: it.next().and_then(|x| x.parse().ok()).unwrap_or(0),
        total: it.next().and_then(|x| x.parse().ok()).unwrap_or(0),
    }
}

/// Parse a Slurm Gres string such as "gpu:a100:4" or "gpu:8" or "(null)".
/// Returns (gpus_per_node, optional_type).
fn parse_gres(s: &str) -> (Option<u32>, Option<String>) {
    let s = s.trim();
    if s.is_empty() || s == "(null)" {
        return (None, None);
    }
    for token in s.split(',') {
        let token = token.trim();
        if !token.starts_with("gpu") {
            continue;
        }
        let parts: Vec<&str> = token.split(':').collect();
        match parts.len() {
            // "gpu" — no count, no type
            1 => return (None, None),
            // "gpu:N"
            2 => return (parts[1].parse().ok(), None),
            // "gpu:type:N" — or "gpu:type:N(IDX:0)" — strip trailing (...)
            _ => {
                let count_token = parts[2].split('(').next().unwrap_or("");
                return (count_token.parse().ok(), Some(parts[1].to_string()));
            }
        }
    }
    (None, None)
}

/// Parse a Slurm memory request (the `%m` field of squeue) into MB.
///
/// Accepts plain numbers (interpreted as MB, which is Slurm's default),
/// or values with K/M/G/T suffixes. Also handles a trailing `n` (per-node)
/// or `c` (per-cpu) suffix by stripping it. Returns `None` for empty,
/// "N/A", or unparseable input.
pub fn parse_slurm_mem(s: &str) -> Option<u64> {
    let s = s.trim();
    if s.is_empty() || s == "N/A" || s == "UNLIMITED" {
        return None;
    }
    let s = s.trim_end_matches(['n', 'c', 'N', 'C']);
    let (num_part, unit) = split_numeric_suffix(s);
    let num: f64 = num_part.parse().ok()?;
    let mb = match unit.to_ascii_uppercase().as_str() {
        "" | "M" => num,
        "K" => num / 1024.0,
        "G" => num * 1024.0,
        "T" => num * 1024.0 * 1024.0,
        _ => return None,
    };
    Some(mb.round().max(0.0) as u64)
}

fn split_numeric_suffix(s: &str) -> (&str, &str) {
    let split = s
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(s.len());
    (&s[..split], &s[split..])
}

/// Parse a Slurm duration string into seconds. Recognized forms:
/// `D-HH:MM:SS`, `HH:MM:SS`, `MM:SS`, `SS`, plus `"UNLIMITED"`/`"N/A"` → None.
pub fn parse_duration(s: &str) -> Option<u64> {
    let s = s.trim();
    if s.is_empty() || s == "UNLIMITED" || s == "N/A" || s == "INVALID" {
        return None;
    }
    let caps = slurm_duration_re().captures(s)?;
    if let Some(secs) = caps.get(5) {
        return secs.as_str().parse().ok();
    }
    let days: u64 = caps
        .get(1)
        .and_then(|m| m.as_str().parse().ok())
        .unwrap_or(0);
    let hours: u64 = caps
        .get(2)
        .and_then(|m| m.as_str().parse().ok())
        .unwrap_or(0);
    let mins: u64 = caps.get(3).and_then(|m| m.as_str().parse().ok())?;
    let secs: u64 = caps.get(4).and_then(|m| m.as_str().parse().ok())?;
    Some(days * 86400 + hours * 3600 + mins * 60 + secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_squeue_text_row() {
        let row = "12345|gpu|train|alice|R|01:02:03|02:00:00|2|nid[0001-0002]|2026-05-19T10:00:00|2026-05-19T10:30:00|gpu:a100:2|16G";
        let jobs = parse_squeue_text(row);
        assert_eq!(jobs.len(), 1);
        let j = &jobs[0];
        assert_eq!(j.job_id, "12345");
        assert_eq!(j.partition, "gpu");
        assert_eq!(j.user, "alice");
        assert_eq!(j.state, JobState::Running);
        assert_eq!(j.elapsed_seconds, Some(3723));
        assert_eq!(j.time_limit_seconds, Some(7200));
        assert_eq!(j.nodes, 2);
        assert!(j.submit_time.is_some());
        assert!(j.start_time.is_some());
        // wait = start - submit = 30 minutes
        let s = j.submit_time.unwrap();
        let t = j.start_time.unwrap();
        assert_eq!((t - s).num_minutes(), 30);
        assert_eq!(j.gres, "gpu:a100:2");
        assert_eq!(j.min_mem_mb, Some(16 * 1024));
    }

    #[test]
    fn parses_slurm_memory_request() {
        assert_eq!(parse_slurm_mem(""), None);
        assert_eq!(parse_slurm_mem("N/A"), None);
        assert_eq!(parse_slurm_mem("4096"), Some(4096));
        assert_eq!(parse_slurm_mem("16G"), Some(16 * 1024));
        assert_eq!(parse_slurm_mem("2T"), Some(2 * 1024 * 1024));
        assert_eq!(parse_slurm_mem("1024K"), Some(1));
        // Slurm appends 'n' (per-node) / 'c' (per-cpu)
        assert_eq!(parse_slurm_mem("8Gn"), Some(8 * 1024));
        assert_eq!(parse_slurm_mem("500Mc"), Some(500));
    }

    #[test]
    fn parses_durations() {
        assert_eq!(parse_duration("00:00:30"), Some(30));
        assert_eq!(parse_duration("01:02:03"), Some(3723));
        assert_eq!(parse_duration("1-00:00:00"), Some(86400));
        assert_eq!(parse_duration("UNLIMITED"), None);
        assert_eq!(parse_duration("30"), Some(30));
    }

    #[test]
    fn parses_sinfo_single_row() {
        let row = "gpu*|12/4/0/16|120/40/0/160|256000|gpu:a100:4";
        let parts = parse_sinfo_text(row);
        assert_eq!(parts.len(), 1);
        let p = &parts[0];
        assert_eq!(p.name, "gpu");
        assert!(p.default);
        assert_eq!(p.nodes.allocated, 12);
        assert_eq!(p.nodes.total, 16);
        assert_eq!(p.cpus.allocated, 120);
        assert_eq!(p.memory_mb_per_node, Some(256000));
        assert_eq!(p.gpus_per_node, Some(4));
        assert_eq!(p.gpu_type.as_deref(), Some("a100"));
    }

    #[test]
    fn parses_sinfo_merges_state_rows() {
        let blob = "cpu|2/0/0/2|16/0/0/16|64000|(null)\ncpu|0/4/0/4|0/32/0/32|64000|(null)\n";
        let parts = parse_sinfo_text(blob);
        assert_eq!(parts.len(), 1);
        let p = &parts[0];
        assert_eq!(p.name, "cpu");
        assert_eq!(p.nodes.allocated, 2);
        assert_eq!(p.nodes.idle, 4);
        assert_eq!(p.nodes.total, 6);
        assert!(p.gpus_per_node.is_none());
    }

    #[test]
    fn parses_scontrol_basic() {
        let blob = "JobId=12345 JobName=train UserId=alice(1000)\n   Partition=gpu Account=lab\n   JobState=RUNNING Reason=None\n   Command=/home/alice/run.sh WorkDir=/home/alice\n";
        let d = parse_scontrol_show_job(blob).unwrap();
        assert_eq!(d.job_id, "12345");
        assert_eq!(d.job_name.as_deref(), Some("train"));
        assert_eq!(d.partition.as_deref(), Some("gpu"));
        assert_eq!(d.state.as_deref(), Some("RUNNING"));
    }
}
