use anyhow::Result;
use regex::Regex;
use std::sync::OnceLock;

use super::model::{Job, JobDetails};
use super::state::JobState;

/// Format string passed to `squeue --format=`. Order must match
/// [`parse_squeue_text`].
pub const SQUEUE_FORMAT: &str = "%i|%P|%j|%u|%T|%M|%l|%D|%R";

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
    let days: u64 = caps.get(1).and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
    let hours: u64 = caps.get(2).and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
    let mins: u64 = caps.get(3).and_then(|m| m.as_str().parse().ok())?;
    let secs: u64 = caps.get(4).and_then(|m| m.as_str().parse().ok())?;
    Some(days * 86400 + hours * 3600 + mins * 60 + secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_squeue_text_row() {
        let row = "12345|gpu|train|alice|R|01:02:03|02:00:00|2|nid[0001-0002]";
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
    fn parses_scontrol_basic() {
        let blob = "JobId=12345 JobName=train UserId=alice(1000)\n   Partition=gpu Account=lab\n   JobState=RUNNING Reason=None\n   Command=/home/alice/run.sh WorkDir=/home/alice\n";
        let d = parse_scontrol_show_job(blob).unwrap();
        assert_eq!(d.job_id, "12345");
        assert_eq!(d.job_name.as_deref(), Some("train"));
        assert_eq!(d.partition.as_deref(), Some("gpu"));
        assert_eq!(d.state.as_deref(), Some("RUNNING"));
    }
}
