# slurmdash

> Terminal user interface for the Slurm workload manager.

`slurmdash` is a terminal dashboard for monitoring and managing Slurm jobs on
remote HPC clusters. It runs on the user's machine, reaches the cluster over
an ordinary SSH connection, and invokes standard Slurm CLI commands. No
software is required on the cluster beyond what Slurm already installs.

---

## Status

Pre-MVP — planning and scaffolding. Nothing usable yet. The [roadmap](#roadmap)
is the working plan; Phase 1 is the next milestone.

Note: Slurm upstream ships an unrelated GTK admin GUI also called `sview` in
`slurm-gui` / `slurm-sview` system packages. `slurmdash` is a separate Rust
project with no relation to it; the name was chosen to avoid the collision.

## Overview

`slurmdash` runs on the user's machine rather than on the cluster login node.
It does not require `slurmrestd`, custom daemons, or any cluster-side install.

- Configuration, themes, and saved cluster profiles are stored locally.
- Commands are issued as `squeue`, `scontrol`, `sacct`, `sinfo`, `scancel`,
  `sbatch`, `sstat`, and `tail`, executed over SSH on the login node.
- Named cluster profiles can be switched without restarting.
- A local SQLite database caches recent command output and stores job and
  resource snapshots for history and charts.
- An optional local web UI (`slurmdash web`) exposes the same dashboard on a
  loopback port for browser-based access.
- Destructive commands (`scancel`, `scontrol hold`, `scontrol release`,
  `scontrol requeue`) show the exact remote command and require confirmation.
  Each call is recorded in a local audit log.
- SSH private keys, passwords, and full logs are not stored. The local
  database can be disabled, encrypted, or wiped.

### Comparison

| Feature                                | slurmdash (planned) | SlurmTUI | turm | s9s  | sltop |
|----------------------------------------|:-------------------:|:--------:|:----:|:----:|:-----:|
| Laptop-first / no server install       | ✅                  | partial  | partial | partial | partial |
| Multi-cluster profiles                 | ✅                  | —        | —    | —    | —     |
| Live job table + colors                | ✅                  | ✅       | ✅   | ✅   | ✅    |
| Job details (`scontrol show job`)      | ✅                  | ✅       | ✅   | ✅   | ✅    |
| Live stdout/stderr tailing             | ✅                  | ✅       | ✅   | ✅   | —     |
| Cancel with confirmation               | ✅                  | ✅       | —    | ✅   | ✅    |
| Hold / release / requeue               | ✅                  | —        | —    | ✅   | —     |
| Progress bars (time / array / queue)   | ✅                  | —        | —    | —    | partial |
| Terminal charts (queue, partition, GPU)| ✅                  | —        | —    | —    | partial |
| Pending-reason explainer               | ✅                  | —        | —    | —    | ✅    |
| Sbatch submission wizard               | ✅                  | —        | —    | —    | —     |
| Local web UI                           | ✅                  | —        | —    | —    | —     |
| Mouse-first controls                   | ✅                  | partial  | —    | partial | ✅ |
| Local desktop notifications            | ✅                  | —        | —    | —    | —     |
| Local SQLite cache + history           | ✅                  | —        | —    | —    | —     |
| Offline review mode                    | ✅                  | —        | —    | —    | —     |
| Built-in LLM prompt assistant          | ✅ (Phase 4)        | —        | —    | —    | —     |
| Smart resource recommendations         | ✅ (Phase 5)        | —        | —    | —    | —     |

---

## Quick start

> Not yet shippable. Once Phase 1 lands:

```sh
# Install
cargo install slurmdash

# Run against a host already in ~/.ssh/config
slurmdash --host login.cluster.edu --user alice

# Or define a profile in ~/.config/slurmdash/config.toml and:
slurmdash connect frontier
```

## Configuration

`~/.config/slurmdash/config.toml`:

```toml
[ui]
theme = "dark"
refresh_seconds = 10
mouse = true
show_charts = true
compact = false

[database]
enabled = true
engine = "sqlite"                                  # sqlite | duckdb (later) | postgres (later)
path = "~/.local/share/slurmdash/slurmdash.db"
auto_migrate = true
retention_days = 90
cache_ttl_seconds = 30
store_logs = false
store_log_excerpts = true
max_log_excerpt_lines = 300
# encryption = "none"                              # none | os_keychain | passphrase | sqlcipher

[clusters.frontier]
host = "login.frontier.olcf.ornl.gov"
user = "alice"
port = 22
ssh_key = "~/.ssh/id_ed25519"
default_account = "project123"
default_partition = "batch"
default_workdir = "/home/alice"

[clusters.lab_cluster]
host = "login.lab.edu"
user = "alice"
default_partition = "cpu"

# Skip SSH entirely — run Slurm commands locally. Useful when running
# slurmdash on the login node itself, or for development.
[clusters.local]
local = true
```

TOML is the source of truth for cluster profiles and base preferences (portable,
version-controllable). Runtime-mutable UI state (last view, column widths,
saved filters) lives in the local database.

## CLI reference

```
slurmdash                                       # open default cluster
slurmdash --cluster NAME                        # named profile
slurmdash --host H --user U [--port N] [--ssh-key PATH]
slurmdash --config PATH

# Filters
slurmdash --me | --all | --user U | --partition P | --account A
slurmdash --state R,PD | --job ID | --name N | --gpu TYPE | --reason R

# Display
slurmdash --refresh SEC | --theme NAME | --compact | --no-mouse
slurmdash --columns jobid,name,state,time_left,reason
slurmdash --sort time_left | --group-by partition
slurmdash --dashboard | --charts

# Subcommands
slurmdash logs JOBID [--stderr] [--follow] [--download]
slurmdash cancel JOBID
slurmdash hold JOBID
slurmdash release JOBID
slurmdash requeue JOBID
slurmdash submit script.sh [--partition P --gres gpu:a100:1 ...]
slurmdash web [--host H] [--port N] [--readonly] [--no-open-browser]

# Local database
slurmdash --db PATH | --no-db | --offline
slurmdash db status
slurmdash db migrate
slurmdash db vacuum
slurmdash db export --format json|csv
slurmdash db clear-cache
slurmdash db clear-history
slurmdash db backup ~/slurmdash-backup.db

# History (powered by local DB)
slurmdash history                                # completed jobs
slurmdash history --offline                      # without cluster connection
slurmdash trends                                 # queue / resource trends over time
```

## Keyboard reference

| Key            | Action                          |
|----------------|---------------------------------|
| `Tab` / `S-Tab`| next / previous panel           |
| `1`–`5`        | dashboard / my jobs / all jobs / logs / resources |
| `Enter`        | open selected job               |
| `l` / `e`      | tail stdout / stderr            |
| `d`            | job details                     |
| `c`            | cancel (with confirmation)      |
| `h` / `r`      | hold / requeue                  |
| `s`            | submit batch script             |
| `/`            | search and filter               |
| `R`            | manual refresh                  |
| `Ctrl+P`       | command palette                 |
| `Ctrl+K`       | prompt assistant (Phase 4)      |
| `?`            | help overlay                    |
| `q`            | quit                            |

Mouse: click column headers to sort, click rows to select, double-click to open
details, scroll tables/logs, click actions in the action bar.

---

## Roadmap

### Phase 1 — MVP

- SSH cluster profiles with `~/.ssh/config` honored
- Live `squeue` table with colors by job state
- Job details panel (parsed from `scontrol show job`)
- Stdout/stderr tail viewer (`tail -f` over SSH, follow + pause + search)
- Cancel / hold / release / requeue (confirm modal, exact command preview)
- Progress bar for running jobs (elapsed vs time limit)
- Sorting, filtering, search
- Mouse support
- Local SQLite DB: schema + migrations, snapshot writer, cache, settings KV, audit log
- Available-resources table (initial `sinfo` view) and node links
- Local-cluster shortcut (no SSH) for dev and login-node use

### Phase 2 — Visual + resource dashboard

- Resource dashboard: CPU / GPU / memory / node state bars
- Partition + QoS overview cards
- Terminal charts driven by stored snapshots: queue size over time, running vs
  pending, GPU usage by partition, runtime histograms
- Array job collapse / expand with progress
- Pending-reason explainer (with fix suggestions)
- Node view with running jobs per node
- Running-jobs-ending-soon widget
- History view (sacct + DB) with runtime / wait / efficiency charts
- Export CSV / JSON; clear-cache / clear-history commands

### Phase 3 — Local web UI

- `slurmdash web --port 8080` serves the same dashboard in a browser
- Token-based local auth, bound to `127.0.0.1` by default
- WebSocket / SSE for live updates (job table, log tail, resource usage)
- Web log viewer + clickable node links + web charts
- Same backend modules as the TUI — the web UI is a thin layer over `slurm/`,
  `ssh/`, and `db/`
- Offline mode (read-only browse of cached data without a live connection)

### Phase 4 — Prompt assistant (LLM)

- Prompt box in terminal and web UI (`Ctrl+K`)
- Generate sbatch scripts from natural-language descriptions
- Generate filters and squeue queries
- Explain pending reasons in context of cluster state
- Explain failures (read logs, exit codes, accounting, suggest a fix)
- Recommend resources / partitions based on local history
- **Every generated command is shown in a preview modal and requires confirmation
  before it touches the cluster.** Audit-logged.
- API keys optional, stored encrypted (OS keychain by default). LLM interactions
  not stored to DB by default.

### Phase 5 — Smart automation

- Automatic walltime suggestion from previous runs of the same job name
- Automatic memory suggestion from previous `MaxRSS`
- Automatic partition suggestion based on wait-time history
- Automatic job-array generation from a template
- Automatic failed-job diagnosis (cross-reference logs, audit, history)
- Automatic requeue suggestion for transient failures
- Warning: every automation surfaces a recommendation; nothing auto-submits
  without explicit confirmation

### Cross-cutting workflow features

These are not their own phase — they land alongside the phases above:

- Sbatch submission wizard (Phase 2/3)
- Multi-cluster switcher in-UI (Phase 1/3)
- Local desktop notifications (Phase 2)
- Dependency graph view (Phase 3)
- Saved filters (Phase 2)
- Command palette (Phase 3)

---

## Architecture

Single Rust binary. Internal modules:

```
src/
├── main.rs            entry, CLI parsing (clap)
├── config/            TOML config + cluster profiles
├── ssh/               session per cluster (openssh crate), tailing
├── slurm/             squeue / scontrol / sacct / sinfo / sbatch wrappers
│   ├── parse.rs       JSON-first with text fallback
│   ├── state.rs       job-state enum, semantic colors
│   └── reason.rs      pending-reason explainer
├── db/                local SQLite (sqlx), migrations, cache, snapshots, audit
│   ├── cache.rs       TTL-bounded cache wrapper around Slurm calls
│   ├── snapshots.rs   job + resource snapshot writer
│   ├── settings.rs    KV-backed runtime settings
│   └── audit.rs       command_audit_log writer
├── app/               app state, event loop, refresh policy
├── tui/               ratatui widgets, theme, layout
│   └── widgets/       job_table, job_details, log_viewer, progress, charts, …
├── web/               (Phase 3) axum server, REST + WebSocket
├── assist/            (Phase 4) LLM prompt assistant + command preview
└── cli/               subcommands (logs, cancel, submit, web, db, history, …)

migrations/             sqlx migration files (versioned SQL)
```

Design rules:

- The `slurm/`, `ssh/`, and `db/` modules know nothing about the UI. The TUI,
  the future web UI, and the LLM assistant consume them as a library.
- Prefer `squeue --json` / `sacct --json` when the cluster's Slurm version
  supports it; fall back to delimited text formats for older clusters.
- One persistent SSH session per cluster (ControlMaster multiplexing via the
  `openssh` crate). Commands ride over the existing TCP connection — no new
  handshake per refresh.
- Cache before the network. The `db/cache.rs` layer wraps every Slurm call:
  short-TTL results (sinfo, partitions, QoS) are served from SQLite to avoid
  hammering the controller. Live `squeue` always re-fetches but its result is
  also written to `job_snapshots` for history.
- Smart refresh: slower when idle, faster after state changes, paused when the
  terminal loses focus, manual via `R`. Defaults to 10 s.
- Every destructive action writes to `command_audit_log` before execution —
  even dry runs.

## Local database

`~/.local/share/slurmdash/slurmdash.db` (SQLite, WAL mode) stores:

| Table                | What                                                    |
|----------------------|---------------------------------------------------------|
| `clusters`           | Mirror of TOML cluster profiles for FK references       |
| `settings`           | KV store for runtime-mutable UI prefs                   |
| `job_snapshots`      | Every job seen on each refresh — beats squeue retention |
| `completed_jobs`     | sacct-mirrored finished jobs with efficiency stats      |
| `resource_snapshots` | Partition/node state over time — powers trend charts    |
| `node_inventory`     | Last-seen node metadata                                 |
| `command_audit_log`  | Every destructive command we ran, with confirmation     |
| `llm_interactions`   | Optional, off by default — prompts + previewed commands |

Privacy defaults:

- Never stores SSH private keys, passwords, or environment variables
- Stores only SSH key *paths*, not key contents
- Full logs not stored — only excerpts (configurable, capped lines)
- LLM prompts not stored unless explicitly opted in
- Optional encryption via OS keychain, passphrase, or SQLCipher
- `slurmdash db clear-cache` / `clear-history` / `--no-db` always available
- `slurmdash db export` and `slurmdash db backup` for portability

Retention defaults: job snapshots 90 d, resource snapshots 30 d, audit 180 d,
cached command output 10 min, log excerpts 14 d. All tunable.

## Color and theming

Themes are first-class. Job state, resource usage, action severity, and
progress bars share a small palette across both terminal and web UI.

| Bucket            | Examples                          | Default color |
|-------------------|-----------------------------------|---------------|
| Running           | `RUNNING`                         | green         |
| Pending           | `PENDING`                         | yellow        |
| Completing        | `COMPLETING`                      | cyan          |
| Completed         | `COMPLETED`                       | blue          |
| Failed / timeout  | `FAILED`, `TIMEOUT`, `NODE_FAIL`  | red           |
| Cancelled         | `CANCELLED`                       | magenta       |
| Preempted         | `PREEMPTED`                       | orange        |
| Held / suspended  | `HELD`, `SUSPENDED`               | purple / gray |

Resource usage bars: green (0–49 %) → yellow (50–79 %) → orange (80–94 %) →
red (95–100 %). Destructive actions are always red and always confirmed.

Built-in themes: `dark` (default), `light`, `high-contrast`, `colorblind-safe`.
Custom themes via `[colors.*]` blocks in `config.toml`.

---

## Building

```sh
cargo build           # debug
cargo build --release # release
cargo test            # unit + parser tests
cargo run -- --help   # see CLI
```

Requires Rust 1.75+ (2024 edition) and the system `ssh` binary on `$PATH`
(OpenSSH 6.7+ for ControlMaster multiplexing).

## License

GPL-3.0. See [LICENSE](LICENSE).
