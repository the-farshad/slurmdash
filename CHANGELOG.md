# Changelog

All notable changes to slurmdash are documented in this file.

The format is loosely based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- `slurmdash config init|show|path` — write a starter config to
  `~/.config/slurmdash/config.toml`, print the resolved config as
  TOML, or print the discovered config path.
- `slurmdash completions <shell>` — generate bash / zsh / fish /
  powershell / elvish completion scripts via `clap_complete`.
- TUI: `a` cycles `filter:me` / `filter:all` at runtime; the header
  shows the active filter next to the cluster name.
- TUI: `/` opens a text-filter input that matches against job id,
  name, user, partition, and reason. The header shows `search:…` in
  the accent color and the job count switches to `shown/total` while
  a filter is active. Esc cancels, Enter commits, Enter on empty
  clears.
- Dashboard top row now includes a **By user** bar chart panel
  showing job counts per user (sorted descending).

### Changed

- TUI refreshes (squeue + sinfo) run on background tasks via an mpsc
  channel, so the event loop never blocks on the network. Keys are
  responsive while the cluster is being polled. The header shows
  `refreshing…` while a fetch is in flight. Initial paint stays
  synchronous.
- `RunnerHandle.runner` is now `Arc<dyn Runner>` so refresh tasks
  can share it across spawns.

### Changed

- README leads with a single Install section that lists per-platform
  tarball one-liners (Linux x86_64, macOS Intel, macOS Apple Silicon)
  plus the build-from-source path.
- Release workflow now builds three targets on tag push: Linux x86_64,
  macOS Intel, macOS Apple Silicon. Each upload includes a sha256
  sidecar.
- CI smoke-tests the built binary with `--version`, `--help`, and a
  walk of every subcommand to catch dispatch-table regressions.

### Fixed

- Removed remaining "laptop" / "frontier" wording from docs and
  examples — replaced with generic placeholders.

## [0.1.0] — 2026-05-19

The first tagged release. Everything described below is included.

### Added

#### Foundation
- `slurmdash` Rust binary (edition 2024, MSRV 1.85). GPL-3.0.
- TOML configuration at `~/.config/slurmdash/config.toml` with named cluster
  profiles. `--host` / `--user` / `--port` / `--ssh-key` overrides. A
  `local = true` profile shortcut runs Slurm commands directly without SSH.
- SSH layer built on the `openssh` crate (ControlMaster multiplexing via the
  system `ssh` binary). All standard OpenSSH features apply: `~/.ssh/config`,
  ssh-agent, ProxyJump, `known_hosts`.
- Local SQLite database at `~/.local/share/slurmdash/slurmdash.db`. Bundled
  sqlx migrations create `clusters`, `settings`, `cache`, `job_snapshots`,
  `resource_snapshots`, `command_audit_log` tables. `slurmdash db
  status / migrate / vacuum / backup / clear-cache / clear-history` all
  implemented.

#### TUI
- ratatui + crossterm dashboard. Default view stacks:
  - Sparkline strip (CPU / GPU / MEM, last 60 samples)
  - Resources / Queue / Ending-soon panels
  - Partition cards with per-partition CPU/GPU/MEM bars
  - Job table with state colors
- Sub-views: jobs-only (`2`), details (`Enter` / `d`), logs (`l` / `e`).
- Keyboard: navigation (`↑↓ jk g G Home End`), sorting (`s` cycles, `S`
  reverses), action keys (`c h u Q`), assist (`Ctrl+K`), help (`?`),
  refresh (`R`), quit (`q`, `Ctrl+C`).
- Mouse: row select on click, scroll-wheel navigation in the job table.
- Confirm modal: every destructive action shows the exact remote command
  and requires Enter/`y` to run; `Esc`/`n` to dismiss.
- Streaming log viewer (`tail -F` over the SSH session) with follow/pause,
  `g`/`G`, `PageUp`/`PageDown`, `/` search + `n` next.
- Pending-reason explainer in the details view.

#### Slurm command wrappers
- `squeue` (text format `%i|%P|%j|%u|%T|%M|%l|%D|%R`)
- `scontrol show job` (parsed into typed `JobDetails`)
- `scancel`, `scontrol hold / release / requeue`
- `sinfo` (text format `%P|%F|%C|%m|%G`, partition-rolled-up with Gres
  parsing for GPU type and count)
- `squeue --version` for version detection
- Pending-reason explainer with summaries + suggestions for common codes
  (`Resources`, `Priority`, `Dependency`, `JobHeldUser`/`JobHeldAdmin`,
  `QOSMaxWallDurationPerJobLimit`, `AssocGrpGRES`/`AssocGrpGPULimit`,
  `ReqNodeNotAvail`, `BeginTime`, `Licenses`, `ReqNodeUnavail`).

#### Local web UI
- `slurmdash web --port 8080` serves the same dashboard in a browser.
- Loopback-bound by default; warns if `--host` moves it off `127.0.0.1`.
- Random per-session token; URL printed on startup. Token accepted as query
  string on first hit, then via cookie / `Authorization: Bearer`.
- Background refresher runs `squeue` + `sinfo` on the configured interval
  and writes snapshots to the same DB the TUI uses.
- REST API: `GET /api/dashboard`, `GET /api/jobs/:id`, `POST
  /api/jobs/:id/{cancel,hold,release,requeue}`, `POST /api/assist`.
- Embedded HTML/CSS/JS (no build step) matches the TUI dark theme. Inline
  per-row cancel/hold buttons open a confirm modal. `Ctrl+K` opens the
  assist dialog.
- `--readonly` flag rejects mutating endpoints with 403.

#### Prompt assistant
- `Ctrl+K` in the TUI and browser; `slurmdash assist "PROMPT" [--job ID]`
  on the CLI.
- Provider trait with two implementations:
  - Ollama (default) — `http://localhost:11434`, `OLLAMA_HOST` /
    `OLLAMA_MODEL` env overrides, tolerates a bare `host:port`.
  - Anthropic — opt-in via `SLURMDASH_LLM_PROVIDER=anthropic` and
    `ANTHROPIC_API_KEY`. Default model `claude-sonnet-4-6`.
- System prompt seeded with cluster name, partition list, job count, the
  selected-job context, and the local history summary for that job's name.
- Proposed commands extracted from the model's text and surfaced as
  numbered options (1–9) in the TUI / "confirm" buttons in the browser;
  each routes through the existing confirm modal, audit-logged.

#### History-driven recommendations
- `slurmdash recommend [--job-name X] [--since-days N]` mines
  `job_snapshots` for per-name run counts, elapsed min/p50/max, and
  failure/timeout/cancellation counts; also reports per-partition
  pending/running averages.
- The TUI details view shows the same summary for the selected job's name
  plus a `--time` suggestion padded 5% above the historical max.
- The CLI and web assist paths inject the history summary into the
  system prompt so the LLM can reason about prior runs without a
  tool-use round-trip.

### Security and privacy

- SSH private keys, passwords, and full logs are never stored locally —
  only key paths and (optionally) log excerpts.
- Web UI is bound to `127.0.0.1` by default; a non-loopback bind emits a
  warning at startup.
- Random session token gates every API endpoint regardless of bind
  address.
- Every destructive remote command is recorded in
  `command_audit_log` (with whether the user confirmed, success, and any
  error) before execution.

### Known limitations

- `--json` paths for `squeue` / `sacct` are not yet used. All parsing
  goes through the delimited text formats; compatibility tested against
  Slurm's standard `--format` strings.
- `completed_jobs` (sacct mirror) is not yet implemented — history
  recommendations work off the snapshot accumulator only.
- Web UI is polled (5 s) rather than SSE / WebSocket.
- No browser log viewer page or per-job detail page yet (API endpoints
  exist).
- Sbatch submission wizard is not yet implemented; `slurmdash submit
  script.sh` is reserved.
- Built and tested on Linux. macOS should work but is not in CI yet.
