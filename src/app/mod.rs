use std::collections::VecDeque;

use ratatui::layout::Rect;

use chrono::{DateTime, Utc};

use crate::actions::ActionKind;
use crate::assist::AssistResponse;
use crate::history::JobNameStats;
use crate::slurm::model::{ClusterResources, Job, JobDetails, Partition};

/// Max samples retained for the in-memory sparkline history. At the default
/// 10-second refresh that's a 10-minute trailing window.
pub const RESOURCE_HISTORY_LIMIT: usize = 60;

const LOG_BUFFER_LIMIT: usize = 5_000;

#[derive(Debug, Default)]
pub struct AppState {
    pub jobs: Vec<Job>,
    pub partitions: Vec<Partition>,
    pub resources: ClusterResources,
    pub resource_history: VecDeque<ResourceSample>,
    pub selected: usize,
    pub view: View,
    pub details: Option<JobDetails>,
    pub details_history: Option<JobNameStats>,
    pub confirm: Option<Confirm>,
    pub show_help: bool,
    pub last_error: Option<String>,
    pub should_quit: bool,

    pub sort: SortState,
    /// Bounds of the job-table widget on the last render, used to translate
    /// mouse clicks into row indices.
    pub table_rect: Option<Rect>,

    pub log: Option<LogView>,
    /// While `Some`, the user is typing into the log search input.
    pub search_input: Option<String>,

    pub assist: Option<AssistDialog>,
}

#[derive(Debug, Default)]
pub struct AssistDialog {
    pub input: String,
    pub response: Option<AssistResponse>,
    pub in_flight: bool,
    pub error: Option<String>,
}

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
pub enum View {
    #[default]
    Dashboard,
    Jobs,
    Details,
    Logs,
}

#[derive(Debug, Clone)]
pub struct Confirm {
    pub kind: ActionKind,
    pub job_id: String,
    pub preview: String,
}

// ---- sorting ---------------------------------------------------------------

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SortKey {
    State,
    JobId,
    Partition,
    User,
    Name,
    Elapsed,
    TimeLimit,
}

impl SortKey {
    pub fn label(self) -> &'static str {
        match self {
            SortKey::State => "state",
            SortKey::JobId => "jobid",
            SortKey::Partition => "partition",
            SortKey::User => "user",
            SortKey::Name => "name",
            SortKey::Elapsed => "elapsed",
            SortKey::TimeLimit => "limit",
        }
    }

    pub fn next(self) -> Self {
        match self {
            SortKey::State => SortKey::JobId,
            SortKey::JobId => SortKey::Partition,
            SortKey::Partition => SortKey::User,
            SortKey::User => SortKey::Name,
            SortKey::Name => SortKey::Elapsed,
            SortKey::Elapsed => SortKey::TimeLimit,
            SortKey::TimeLimit => SortKey::State,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SortState {
    pub key: SortKey,
    pub reverse: bool,
}

impl Default for SortState {
    fn default() -> Self {
        Self {
            key: SortKey::State,
            reverse: false,
        }
    }
}

pub fn apply_sort(jobs: &mut Vec<Job>, sort: SortState) {
    use std::cmp::Ordering;
    jobs.sort_by(|a, b| {
        let ord = match sort.key {
            SortKey::State => a.state.short().cmp(b.state.short()),
            SortKey::JobId => natural_cmp(&a.job_id, &b.job_id),
            SortKey::Partition => a.partition.cmp(&b.partition),
            SortKey::User => a.user.cmp(&b.user),
            SortKey::Name => a.name.cmp(&b.name),
            SortKey::Elapsed => a.elapsed_seconds.cmp(&b.elapsed_seconds),
            SortKey::TimeLimit => a.time_limit_seconds.cmp(&b.time_limit_seconds),
        };
        if sort.reverse { ord.reverse() } else { ord }
    });
    // Stability: secondary sort by job_id so the order is deterministic when
    // the primary key ties.
    let _ = Ordering::Equal;
}

fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    match (a.parse::<u64>(), b.parse::<u64>()) {
        (Ok(x), Ok(y)) => x.cmp(&y),
        _ => a.cmp(b),
    }
}

// ---- log viewer ------------------------------------------------------------

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum LogKind {
    Stdout,
    Stderr,
}

impl LogKind {
    pub fn label(self) -> &'static str {
        match self {
            LogKind::Stdout => "stdout",
            LogKind::Stderr => "stderr",
        }
    }
}

#[derive(Debug)]
pub struct LogView {
    pub job_id: String,
    pub kind: LogKind,
    pub path: String,
    pub lines: VecDeque<String>,
    /// If `follow` is true, the viewer keeps the bottom in view as new lines
    /// arrive. Toggled with `f`.
    pub follow: bool,
    /// Top line index when not following.
    pub scroll: usize,
    /// Active search query (post-Enter); used by `n` / `N` to navigate.
    pub search: Option<String>,
}

impl LogView {
    pub fn new(job_id: String, kind: LogKind, path: String) -> Self {
        Self {
            job_id,
            kind,
            path,
            lines: VecDeque::with_capacity(1024),
            follow: true,
            scroll: 0,
            search: None,
        }
    }

    pub fn push(&mut self, line: String) {
        if self.lines.len() == LOG_BUFFER_LIMIT {
            self.lines.pop_front();
            self.scroll = self.scroll.saturating_sub(1);
        }
        self.lines.push_back(line);
    }

    pub fn scroll_to_bottom(&mut self, visible_rows: usize) {
        self.scroll = self.lines.len().saturating_sub(visible_rows);
    }

    pub fn scroll_by(&mut self, delta: i32) {
        let new = (self.scroll as i32 + delta).max(0) as usize;
        self.scroll = new.min(self.lines.len().saturating_sub(1));
        self.follow = false;
    }

    pub fn find_next(&mut self, from: usize) -> Option<usize> {
        let q = self.search.as_deref()?;
        if q.is_empty() {
            return None;
        }
        for (i, line) in self.lines.iter().enumerate().skip(from) {
            if line.contains(q) {
                self.scroll = i;
                self.follow = false;
                return Some(i);
            }
        }
        None
    }
}

// ---- AppState methods ------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub struct ResourceSample {
    pub when: DateTime<Utc>,
    pub cpu_pct: f32,
    pub gpu_pct: f32,
    pub mem_pct: f32,
    pub has_gpu: bool,
}

impl ResourceSample {
    pub fn from(now: DateTime<Utc>, r: &ClusterResources) -> Self {
        Self {
            when: now,
            cpu_pct: r.cpus.pct_allocated() as f32,
            gpu_pct: r.gpus.pct_allocated() as f32,
            mem_pct: r.memory_mb.pct_allocated() as f32,
            has_gpu: r.gpus.total > 0,
        }
    }
}

impl AppState {
    pub fn push_resource_sample(&mut self, sample: ResourceSample) {
        if self.resource_history.len() == RESOURCE_HISTORY_LIMIT {
            self.resource_history.pop_front();
        }
        self.resource_history.push_back(sample);
    }

    pub fn select_next(&mut self) {
        if self.jobs.is_empty() {
            self.selected = 0;
            return;
        }
        self.selected = (self.selected + 1).min(self.jobs.len() - 1);
    }

    pub fn select_prev(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn selected_job(&self) -> Option<&Job> {
        self.jobs.get(self.selected)
    }
}
