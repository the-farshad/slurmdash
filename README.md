# slurmdash

> Terminal user interface for the Slurm workload manager.

`slurmdash` runs on your laptop and gives you a live dashboard for the
clusters you SSH into — job table, resource bars, log tailing, safe
cancel/hold/release, a built-in LLM assistant, and an optional local web
UI. No software is required on the cluster beyond what Slurm itself
already installs.

- **Version:** 0.1.0 — see [CHANGELOG.md](CHANGELOG.md)
- **Source:** [github.com/the-farshad/slurmdash](https://github.com/the-farshad/slurmdash)
- **License:** GPL-3.0

Not to be confused with Slurm upstream's own `sview` (GTK admin GUI from
SchedMD). `slurmdash` is a separate Rust project; the name was chosen to
avoid that collision.

---

## Tour

The block-character panels below are real renderings of what each view
looks like. PNG captures from live terminals can be dropped into
[`docs/screenshots/`](docs/screenshots/) when you take them.

### Dashboard (default view)

```text
┌ slurmdash  frontier  42 jobs  updated 14:23:01  sort:state↑ ──────────────────────┐
│                                                                                    │
│ ┌ History (last 60 samples) ──────────────────────────────────────────────────┐    │
│ │ CPU  ▁▂▃▄▅▆▇█▇▆▆▅▅▄▄▃▃▂▂  67%   GPU  ▄▄▅▅▆▆▇█  82%   MEM  ▁▁▂▂▃▃▄▄  56%  │    │
│ └─────────────────────────────────────────────────────────────────────────────┘    │
│                                                                                    │
│ ┌ Resources ──────────┐ ┌ Queue ──────────┐ ┌ Ending soon ─────────────────────┐  │
│ │ CPU  [████████░░] 67% │ │ R   ████████ 12 │ │ 12345 train         -02:15      │  │
│ │ GPU  [█████████░] 82% │ │ PD  █████    5  │ │ 12346 inference     -08:42      │  │
│ │ MEM  [██████░░░░] 56% │ │ CD  ██       2  │ │ 12350 preprocess    -22:18      │  │
│ │ NODE alloc:12 idle:4  │ │ F   █        1  │ │                                 │  │
│ └───────────────────────┘ └─────────────────┘ └─────────────────────────────────┘  │
│                                                                                    │
│ ┌ Partitions ───────────────────────────────────────────────────────────────────┐  │
│ │ gpu-a100   cpu ████████░ 80%  gpu ██████░░ 60%  mem ████░░░░ 40%  12/16 nodes │  │
│ │ gpu-h100   cpu ███████░░ 75%  gpu █████████ 90%  mem ███████░ 70%  8/8  nodes │  │
│ │ cpu        cpu ███░░░░░░ 34%  mem ███░░░░░░ 30%                   4/12 nodes │  │
│ └───────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                    │
│ ┌ JOBID    PART      NAME           USER     ST  ELAPSED   LIMIT     N  REASON  ┐  │
│ │ 12345    gpu-a100  train_resnet50 alice    R   02:15:00  04:00:00  2  nid001  │  │
│ │ 12346    gpu-h100  inference      bob      R   08:42:00  12:00:00  1  nid002  │  │
│ │ 12347    cpu       preprocess     alice    PD  --        02:00:00  1  Priority│  │
│ │ 12348    gpu-a100  finetune       carol    PD  --        06:00:00  4  Resources│ │
│ └───────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                    │
│ 1 dash  2 jobs  ↑↓ select  Enter details  l logs  c cancel  ^K assist  ? help  q  │
└────────────────────────────────────────────────────────────────────────────────────┘
```

### Job details (Enter / d)

```text
┌ slurmdash  frontier  42 jobs ─────────────────────────────────────────────────────┐
│                                                                                    │
│ Time   [████████████░░░░░░░░] 52%  02:15:00 / 04:00:00                            │
│                                                                                    │
│  Job 12345                                                                         │
│                                                                                    │
│    Name        train_resnet50                                                      │
│    User        alice                                                               │
│    Account     ml-lab                                                              │
│    Partition   gpu-a100                                                            │
│    State       RUNNING                                                             │
│    Command     /home/alice/run.sh                                                  │
│    WorkDir     /home/alice/exps/resnet50-run-12                                    │
│    StdOut      /home/alice/exps/resnet50-run-12/slurm-12345.out                    │
│    StdErr      /home/alice/exps/resnet50-run-12/slurm-12345.err                    │
│    NodeList    nid001                                                              │
│                                                                                    │
│  History                                                                           │
│    train_resnet50: 12 runs, median elapsed 2h14m, max 3h47m                        │
│    suggest --time at least 3h59m (median 2h14m / max 3h47m)                        │
│                                                                                    │
│ Esc back   q quit                                                                  │
└────────────────────────────────────────────────────────────────────────────────────┘
```

A pending job replaces the progress bar with a `Reason` line — for
example `Reason  Priority — Other jobs currently have higher
scheduling priority.`

### Log viewer (l for stdout, e for stderr)

```text
┌ log  stdout  /home/alice/exps/resnet50-run-12/slurm-12345.out  FOLLOW  3812 lines ─┐
│                                                                                    │
│   Epoch  9/40   loss 1.243   acc 0.612   lr 0.0010   eta 1:47:00                  │
│   Epoch 10/40   loss 1.187   acc 0.628   lr 0.0010   eta 1:36:21                  │
│   Epoch 11/40   loss 1.132   acc 0.641   lr 0.0009   eta 1:25:55                  │
│   [val] step 50  loss 1.094  acc 0.659                                            │
│   Epoch 12/40   loss 1.078   acc 0.658   lr 0.0009   eta 1:15:32                  │
│ ▌ Epoch 13/40   loss 1.025   acc 0.674   lr 0.0008   eta 1:05:14                  │
│   …                                                                                │
│                                                                                    │
│ ↑↓ jk scroll  PgUp/PgDn page  g top  G bottom  f follow  / search  n next  Esc    │
└────────────────────────────────────────────────────────────────────────────────────┘
```

`/` opens a search buffer; matched substrings are highlighted in the
accent color. Press `f` to pause autoscroll.

### Confirm modal (c cancel, h hold, u release, Q requeue)

```text
                         ┌ Confirm ───────────────────────────┐
                         │ cancel job 12345                   │
                         │                                    │
                         │ $ scancel 12345                    │
                         │                                    │
                         │ Enter / y to confirm    Esc / n    │
                         └────────────────────────────────────┘
```

Every destructive remote command goes through this prompt, regardless of
whether it was triggered from a hotkey, the LLM assistant, or the web UI.
Each call is recorded in `command_audit_log`.

### Assist (Ctrl+K) — defaults to local Ollama

```text
┌ Assist (Ctrl+K) ───────────────────────────────────────────────────────────────────┐
│ ┌ prompt ────────────────────────────────────────────────────────────────────────┐ │
│ │ my job 12347 is pending — what's the fastest fix?_                             │ │
│ └────────────────────────────────────────────────────────────────────────────────┘ │
│ ┌────────────────────────────────────────────────────────────────────────────────┐ │
│ │  [ollama · llama3.2]                                                           │ │
│ │                                                                                │ │
│ │  Reason=Priority means other jobs in this partition outrank yours. You can:    │ │
│ │   - wait it out (your fairshare will recover)                                  │ │
│ │   - try the cpu partition if your job doesn't actually need a GPU              │ │
│ │   - hold and resubmit with a tighter --time so the scheduler can backfill      │ │
│ │                                                                                │ │
│ │   proposed commands (press 1-9 to confirm)                                     │ │
│ │   1. scontrol hold 12347                                                       │ │
│ └────────────────────────────────────────────────────────────────────────────────┘ │
│ Enter send   1-9 confirm command   Esc close                                       │
└────────────────────────────────────────────────────────────────────────────────────┘
```

Pressing `1` opens the standard Confirm modal with `scontrol hold 12347`
as the preview. The LLM never runs commands directly — every action is
gated on user confirmation and audit-logged.

### Local web UI (`slurmdash web`)

```text
┌──────────────────────────────────────────────────────────────────────────────┐
│ slurmdash  frontier   updated 14:23:01                              [readonly]│
├──────────────────────────────────────────────────────────────────────────────┤
│ ┌─ Resources ────────────┐ ┌─ Queue ──────────┐ ┌─ Ending soon ───────────┐  │
│ │ CPU  ▰▰▰▰▰▰▱▱▱▱  67%   │ │ R   ▰▰▰▰▰▰▰▰ 12  │ │ 12345 train     -02:15 │  │
│ │ GPU  ▰▰▰▰▰▰▰▰▰▱  82%   │ │ PD  ▰▰▰▰▰     5  │ │ 12346 inference -08:42 │  │
│ │ MEM  ▰▰▰▰▰▰▱▱▱▱  56%   │ │ CD  ▰▰        2  │ │ …                       │  │
│ └────────────────────────┘ └──────────────────┘ └─────────────────────────┘  │
│                                                                              │
│ ┌─ Partitions ──────────────────────────────────────────────────────────────┐│
│ │ gpu-a100   cpu ▰▰▰▰▰▰▰▰▱▱ 80%  gpu ▰▰▰▰▰▰▱▱▱▱ 60%  mem ▰▰▰▰▱▱▱▱▱▱ 40% ││
│ │ cpu        cpu ▰▰▰▱▱▱▱▱▱▱ 34%  mem ▰▰▰▱▱▱▱▱▱▱ 30%                       ││
│ └────────────────────────────────────────────────────────────────────────────┘│
│                                                                              │
│ ┌─ Jobs ────────────────────────────────────────────────────────────────────┐│
│ │ Job    Part      Name           User    State  Elapsed  Limit    Actions ││
│ │ 12345  gpu-a100  train_resnet50 alice   R      2:15:00  4:00:00  cancel ││
│ │ 12346  gpu-h100  inference      bob     R      8:42:00  12:00:00 cancel ││
│ │ 12347  cpu       preprocess     alice   PD                       hold   ││
│ └────────────────────────────────────────────────────────────────────────────┘│
│ auto-refresh every 5s · keys: r refresh now · Ctrl+K assist · Esc close      │
└──────────────────────────────────────────────────────────────────────────────┘
```

Same backend modules as the TUI. The browser polls `/api/dashboard`
every five seconds and feeds the same audit-logged confirm flow when you
click a destructive button.

---

## Install

Requires Rust **1.85+** (2024 edition) and the system `ssh` binary on
`$PATH`.

From source — recommended until the crate is published:

```sh
git clone https://github.com/the-farshad/slurmdash
cd slurmdash
cargo install --path .
```

Or grab the prebuilt Linux x86_64 binary from the
[latest release](https://github.com/the-farshad/slurmdash/releases/latest).

Future:

```sh
cargo install slurmdash
```

## Quick start

```sh
# 1. Talk to a host already in your ~/.ssh/config
slurmdash --host login.cluster.edu --user alice

# 2. Or define cluster profiles in ~/.config/slurmdash/config.toml
slurmdash connect frontier

# 3. Browser dashboard on a loopback port
slurmdash web --port 8080

# 4. CLI one-shots
slurmdash cancel 12345
slurmdash assist "why is my job pending?" --job 12347
slurmdash recommend --job-name train_resnet50
```

## Configuration

Path: `~/.config/slurmdash/config.toml`. Minimal example:

```toml
[ui]
theme = "dark"                 # dark | light | high-contrast | colorblind-safe
refresh_seconds = 10
mouse = true

[database]
enabled = true
retention_days = 90

[clusters.frontier]
host = "login.frontier.olcf.ornl.gov"
user = "alice"
ssh_key = "~/.ssh/id_ed25519"
default_partition = "batch"
default_account = "project123"

[clusters.local]
local = true                   # bypass SSH and run Slurm commands directly
```

Cluster profiles are the source of truth for connection info;
runtime-mutable UI state (column widths, last view) lives in the local
database.

## Keyboard reference

| Key            | Where      | Action                                       |
|----------------|------------|----------------------------------------------|
| `1` / `2`      | global     | dashboard / plain jobs view                  |
| `↑` / `↓` / `j` / `k` | jobs       | select previous / next job            |
| `g` / `G`      | jobs/logs  | jump to top / bottom                         |
| `Enter` / `d`  | jobs       | open job details                             |
| `l` / `e`      | jobs       | open stdout / stderr log viewer              |
| `c`            | jobs       | cancel selected (confirm modal)              |
| `h` / `u` / `Q`| jobs       | hold / release / requeue (confirm modal)     |
| `s` / `S`      | jobs       | cycle sort key / reverse                     |
| `R` / `r`      | jobs/logs  | refresh now                                  |
| `f`            | logs       | toggle follow                                |
| `/`            | logs       | search; `n` next match                       |
| `Esc`          | any modal  | close modal / back to dashboard              |
| `Ctrl+K`       | global     | open assist (LLM)                            |
| `1`–`9`        | assist     | confirm Nth proposed command                 |
| `?`            | jobs/logs  | help overlay                                 |
| `q` / `Ctrl+C` | global     | quit                                         |

Mouse: row click selects, wheel scrolls selection.

## CLI reference (subcommands)

```text
slurmdash                    # open the TUI
slurmdash connect NAME       # open the TUI for a named cluster profile
slurmdash logs JOBID [--stderr] [--follow]
slurmdash cancel JOBID
slurmdash hold JOBID
slurmdash release JOBID
slurmdash requeue JOBID
slurmdash submit script.sh   # (reserved for the sbatch wizard, not yet implemented)
slurmdash assist "PROMPT" [--job JOBID]
slurmdash recommend [--job-name X] [--since-days N]
slurmdash history            # (reserved)
slurmdash trends             # (reserved)
slurmdash web [--port N] [--host ADDR] [--readonly] [--no-open-browser]
slurmdash db status | migrate | vacuum | backup PATH | clear-cache | clear-history
```

Common flags: `--cluster NAME`, `--host`, `--user`, `--port`,
`--ssh-key`, `--config PATH`, `--db PATH`, `--no-db`, `--offline`,
`--me`/`--all`, `--partition P`, `--state R,PD`, `--refresh SEC`,
`--theme NAME`.

## LLM assistant

`Ctrl+K` in the TUI / browser, or `slurmdash assist "prompt"` on the CLI.

- **Default:** Ollama on `http://localhost:11434`. Override the model with
  `OLLAMA_MODEL` (default `llama3.2`). `OLLAMA_HOST=host:port` also works
  without an `http://` prefix.
- **Opt-in:** Anthropic via `SLURMDASH_LLM_PROVIDER=anthropic` and
  `ANTHROPIC_API_KEY` (default model `claude-sonnet-4-6`,
  `ANTHROPIC_MODEL` to override).
- The system prompt is seeded with the current cluster snapshot, the
  selected-job context, and (when a DB is enabled) a one-line history
  summary for the job's name.
- Any command the model proposes is shown in the standard Confirm modal
  before it runs and recorded in `command_audit_log`. The model never
  executes commands directly.

## Local database

`~/.local/share/slurmdash/slurmdash.db` (SQLite, WAL mode). Tables:

| Table                | Purpose                                                |
|----------------------|--------------------------------------------------------|
| `clusters`           | FK target for snapshots and audit entries              |
| `settings`           | KV store for runtime-mutable UI prefs                  |
| `cache`              | TTL-bounded results of expensive Slurm calls           |
| `job_snapshots`      | Every job seen on each refresh                         |
| `resource_snapshots` | Per-partition snapshots from sinfo                     |
| `command_audit_log`  | Every destructive command, with confirmation + result  |

Defaults: 90-day job retention, 30-day resource retention, 180-day audit,
10-minute cache TTL. Run `slurmdash db status` to inspect, `clear-cache`
/ `clear-history` to wipe, `backup PATH` to copy.

Privacy: SSH private keys, passwords, and full logs are never stored.
The whole database can be disabled with `--no-db`.

## Architecture

```text
src/
├── main.rs / lib.rs   entry, async runtime, tracing
├── cli.rs             clap subcommands and dispatch
├── config.rs          TOML config + cluster profiles
├── ssh/               Runner trait + LocalRunner + RemoteRunner (openssh)
├── slurm/             squeue / scontrol / sacct / sinfo / sbatch wrappers
├── db/                sqlx + bundled migrations + snapshot/audit writers
├── history/           deterministic recommendation analyzers
├── actions.rs         confirm-modal-friendly destructive-action dispatcher
├── assist/            Ollama (default) + Anthropic providers, prompt builder
├── app/               app state, sorting, log view, assist dialog
├── tui/               ratatui widgets (dashboard, details, logs, modals, …)
├── web/               axum server, REST API, embedded HTML/CSS/JS
└── error.rs

migrations/             bundled SQL files
assets/web/             embedded HTML / CSS / JS for the web UI
```

The `slurm`, `ssh`, `db`, and `history` modules know nothing about the UI
— the TUI, the web UI, and the LLM assistant are thin layers over the
same core.

## Status and roadmap

Released versions and what's in them live in [CHANGELOG.md](CHANGELOG.md).

Open follow-ups for 0.2.x:

- `cargo publish` to crates.io
- macOS / Windows binaries via CI cross-compile
- Sacct mirror → richer history and MaxRSS-based memory suggestions
- Web UI: SSE / WebSocket live updates, browser log viewer, per-job page
- Sbatch submission wizard
- Node-level view, array-job collapse, dependency graph

## Building from source

```sh
cargo build              # debug
cargo build --release    # release (used for distribution)
cargo test               # 5 unit + 2 integration tests
cargo run -- --help      # see the CLI
cargo fmt --all          # rustfmt
cargo clippy --all-targets --no-deps -- -D warnings
```

CI runs the same fmt / clippy / test triad on every push and PR; see
[`.github/workflows/ci.yml`](.github/workflows/ci.yml).

## License

GPL-3.0. See [LICENSE](LICENSE).
