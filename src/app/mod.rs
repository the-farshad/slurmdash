use std::collections::{BTreeMap, HashSet, VecDeque};

use ratatui::layout::Rect;

use chrono::{DateTime, Utc};

use crate::actions::ActionKind;
use crate::assist::AssistResponse;
use crate::history::JobNameStats;
use crate::slurm::model::{ClusterResources, Job, JobDetails, Partition};

/// Which slice of the queue the dashboard is showing.
#[derive(Debug, Clone, Default)]
pub enum FilterMode {
    /// `squeue --me` — only the current SSH user's jobs.
    #[default]
    Me,
    /// `squeue` with no user filter — everyone's jobs.
    All,
    /// `squeue --user <U>` — a single named user.
    User(String),
}

impl FilterMode {
    pub fn label(&self) -> String {
        match self {
            FilterMode::Me => "me".to_string(),
            FilterMode::All => "all".to_string(),
            FilterMode::User(u) => format!("user={u}"),
        }
    }

    /// `a` cycles Me → All → Me. User(_) → Me (one-way exit from a custom filter).
    pub fn cycle(&self) -> Self {
        match self {
            FilterMode::Me => FilterMode::All,
            FilterMode::All => FilterMode::Me,
            FilterMode::User(_) => FilterMode::Me,
        }
    }
}

/// Max samples retained for the in-memory sparkline history. At the default
/// 10-second refresh that's a 10-minute trailing window.
pub const RESOURCE_HISTORY_LIMIT: usize = 60;

const LOG_BUFFER_LIMIT: usize = 5_000;

#[derive(Debug, Default)]
pub struct AppState {
    /// The full job list from the most recent squeue refresh (pre-filter).
    pub all_jobs: Vec<Job>,
    /// The displayed list — `all_jobs` after applying `text_filter` and
    /// the active sort. Selection / rendering operate on this.
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
    pub filter: FilterMode,
    /// Committed free-text filter (set by Enter in the `/` input).
    pub text_filter: Option<String>,
    /// While `Some`, the user is typing into the `/` filter input.
    pub filter_input: Option<String>,
    /// Background refresh state — set when a squeue/sinfo task is in flight.
    pub refresh: RefreshFlags,
    /// Grouping mode (Tab cycles).
    pub group_by: GroupBy,
    /// Group keys the user has collapsed (Enter on a group header).
    pub collapsed_groups: HashSet<String>,
    /// Cached row layout for the job table. Rebuilt whenever jobs / filter /
    /// sort / group state changes.
    pub display_rows: Vec<DisplayRow>,
    /// Bounds of the job-table widget on the last render, used to translate
    /// mouse clicks into row indices.
    pub table_rect: Option<Rect>,
    /// Frame counter for animating the loading spinner.
    pub frame: u64,

    pub log: Option<LogView>,
    /// While `Some`, the user is typing into the log search input.
    pub search_input: Option<String>,

    pub assist: Option<AssistDialog>,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct RefreshFlags {
    pub jobs_in_flight: bool,
    pub sinfo_in_flight: bool,
}

/// What `Tab` cycles through. Groups in the job table by the chosen field.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
pub enum GroupBy {
    /// Flat table, no grouping.
    #[default]
    None,
    User,
    Partition,
    State,
}

impl GroupBy {
    pub fn cycle(self) -> Self {
        match self {
            GroupBy::None => GroupBy::User,
            GroupBy::User => GroupBy::Partition,
            GroupBy::Partition => GroupBy::State,
            GroupBy::State => GroupBy::None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            GroupBy::None => "flat",
            GroupBy::User => "user",
            GroupBy::Partition => "partition",
            GroupBy::State => "state",
        }
    }
}

/// One renderable row in the job table. Drives both rendering and
/// selection navigation when grouping is on.
#[derive(Debug, Clone)]
pub enum DisplayRow {
    Group {
        key: String,
        count: u32,
        collapsed: bool,
    },
    /// Index into [`AppState::jobs`].
    JobIndex(usize),
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

pub fn apply_sort(jobs: &mut [Job], sort: SortState) {
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

fn matches_text_filter(j: &Job, filter: Option<&str>) -> bool {
    let Some(q) = filter else { return true };
    let q = q.trim();
    if q.is_empty() {
        return true;
    }
    let q = q.to_lowercase();
    j.job_id.to_lowercase().contains(&q)
        || j.name.to_lowercase().contains(&q)
        || j.user.to_lowercase().contains(&q)
        || j.partition.to_lowercase().contains(&q)
        || j.reason_or_nodelist.to_lowercase().contains(&q)
}

fn build_grouped<F: Fn(&Job) -> String>(
    jobs: &[Job],
    collapsed: &HashSet<String>,
    field: F,
) -> Vec<DisplayRow> {
    let mut groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, j) in jobs.iter().enumerate() {
        groups.entry(field(j)).or_default().push(i);
    }
    let mut out = Vec::new();
    for (key, members) in groups {
        let count = members.len() as u32;
        let collapsed = collapsed.contains(&key);
        out.push(DisplayRow::Group {
            key,
            count,
            collapsed,
        });
        if !collapsed {
            for idx in members {
                out.push(DisplayRow::JobIndex(idx));
            }
        }
    }
    out
}

impl AppState {
    /// Rebuild `jobs` from `all_jobs` using the current `text_filter`,
    /// reapply `sort`, regenerate `display_rows`, and clamp the selection.
    /// Call after `all_jobs` / `text_filter` / `sort` / `group_by` /
    /// `collapsed_groups` changes.
    pub fn rebuild_filtered_jobs(&mut self) {
        let filter = self.text_filter.as_deref();
        self.jobs = self
            .all_jobs
            .iter()
            .filter(|j| matches_text_filter(j, filter))
            .cloned()
            .collect();
        apply_sort(&mut self.jobs, self.sort);
        self.rebuild_display_rows();
    }

    /// Recompute [`display_rows`] from `jobs` + `group_by` + `collapsed_groups`.
    pub fn rebuild_display_rows(&mut self) {
        self.display_rows = match self.group_by {
            GroupBy::None => (0..self.jobs.len()).map(DisplayRow::JobIndex).collect(),
            GroupBy::User => build_grouped(&self.jobs, &self.collapsed_groups, |j| j.user.clone()),
            GroupBy::Partition => {
                build_grouped(&self.jobs, &self.collapsed_groups, |j| j.partition.clone())
            }
            GroupBy::State => build_grouped(&self.jobs, &self.collapsed_groups, |j| {
                j.state.short().to_string()
            }),
        };
        if !self.display_rows.is_empty() && self.selected >= self.display_rows.len() {
            self.selected = self.display_rows.len() - 1;
        }
    }

    /// Toggle collapse state of the group at the selected row, if any.
    /// Returns true if a group was toggled.
    pub fn toggle_selected_group(&mut self) -> bool {
        let key_opt = match self.display_rows.get(self.selected) {
            Some(DisplayRow::Group { key, .. }) => Some(key.clone()),
            _ => None,
        };
        let Some(key) = key_opt else { return false };
        if !self.collapsed_groups.insert(key.clone()) {
            self.collapsed_groups.remove(&key);
        }
        self.rebuild_display_rows();
        true
    }

    pub fn push_resource_sample(&mut self, sample: ResourceSample) {
        if self.resource_history.len() == RESOURCE_HISTORY_LIMIT {
            self.resource_history.pop_front();
        }
        self.resource_history.push_back(sample);
    }

    pub fn select_next(&mut self) {
        let n = self.display_rows.len();
        if n == 0 {
            self.selected = 0;
            return;
        }
        self.selected = (self.selected + 1).min(n - 1);
    }

    pub fn select_prev(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn select_page_down(&mut self, page: usize) {
        let n = self.display_rows.len();
        if n == 0 {
            return;
        }
        self.selected = (self.selected + page).min(n - 1);
    }

    pub fn select_page_up(&mut self, page: usize) {
        self.selected = self.selected.saturating_sub(page);
    }

    /// The Job at the current selection, if the selected row is a job row
    /// (group headers return None).
    pub fn selected_job(&self) -> Option<&Job> {
        match self.display_rows.get(self.selected)? {
            DisplayRow::JobIndex(idx) => self.jobs.get(*idx),
            _ => None,
        }
    }
}
