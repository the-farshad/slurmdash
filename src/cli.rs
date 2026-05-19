use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "slurmdash",
    version,
    about = "Terminal user interface for the Slurm workload manager",
    long_about = None,
)]
pub struct Cli {
    /// Cluster profile name from config
    #[arg(long, env = "SLURMDASH_CLUSTER")]
    pub cluster: Option<String>,

    /// SSH host (overrides cluster profile)
    #[arg(long)]
    pub host: Option<String>,

    /// SSH user
    #[arg(long)]
    pub user: Option<String>,

    /// SSH port
    #[arg(long)]
    pub port: Option<u16>,

    /// Path to SSH private key
    #[arg(long, value_name = "PATH")]
    pub ssh_key: Option<PathBuf>,

    /// Path to config file
    #[arg(long, value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// Path to local SQLite database
    #[arg(long, value_name = "PATH", conflicts_with = "no_db")]
    pub db: Option<PathBuf>,

    /// Disable the local database
    #[arg(long)]
    pub no_db: bool,

    /// Read only from the local database; do not contact the cluster
    #[arg(long)]
    pub offline: bool,

    /// Show only the current user's jobs
    #[arg(long, conflicts_with = "all")]
    pub me: bool,

    /// Show all users' jobs
    #[arg(long, conflicts_with = "me")]
    pub all: bool,

    /// Filter by partition
    #[arg(long)]
    pub partition: Option<String>,

    /// Filter by job state (comma-separated, e.g. R,PD)
    #[arg(long)]
    pub state: Option<String>,

    /// Refresh interval in seconds
    #[arg(long, value_name = "SECONDS")]
    pub refresh: Option<u64>,

    /// Theme name (dark, light, high-contrast, colorblind-safe)
    #[arg(long)]
    pub theme: Option<String>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Connect to a saved cluster profile (same as --cluster)
    Connect { cluster: String },

    /// Tail job logs (stdout by default)
    Logs {
        job_id: String,
        #[arg(long)]
        stderr: bool,
        #[arg(long)]
        follow: bool,
        #[arg(long)]
        download: bool,
    },

    /// Cancel a job (scancel)
    Cancel { job_id: String },

    /// Hold a job (scontrol hold)
    Hold { job_id: String },

    /// Release a held job (scontrol release)
    Release { job_id: String },

    /// Requeue a job (scontrol requeue)
    Requeue { job_id: String },

    /// Submit a batch script (sbatch)
    Submit {
        script: PathBuf,
        #[arg(long)]
        partition: Option<String>,
        #[arg(long)]
        account: Option<String>,
        #[arg(long)]
        gres: Option<String>,
    },

    /// Show job history from the local database
    History {
        #[arg(long)]
        offline: bool,
    },

    /// Show resource trends from the local database
    Trends,

    /// Local database management
    Db {
        #[command(subcommand)]
        cmd: DbCommand,
    },

    /// Ask the configured LLM provider for help
    Assist {
        /// The prompt to send
        prompt: String,
        /// Selected job id for additional context
        #[arg(long)]
        job: Option<String>,
    },

    /// Show history-driven recommendations from the local database
    Recommend {
        /// Filter to one job name
        #[arg(long)]
        job_name: Option<String>,
        /// Trailing window in days
        #[arg(long, default_value_t = 30)]
        since_days: i64,
    },

    /// Local config-file helpers
    Config {
        #[command(subcommand)]
        cmd: ConfigCmd,
    },

    /// Print shell completions to stdout
    Completions {
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },

    /// Local web UI (Phase 3 — not yet implemented)
    Web {
        #[arg(long)]
        host: Option<String>,
        #[arg(long)]
        port: Option<u16>,
        #[arg(long)]
        readonly: bool,
        #[arg(long)]
        no_open_browser: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum ConfigCmd {
    /// Write a starter config to ~/.config/slurmdash/config.toml
    Init {
        /// Overwrite an existing file
        #[arg(long)]
        force: bool,
        /// Write to this path instead of the default
        #[arg(long, value_name = "PATH")]
        path: Option<PathBuf>,
    },
    /// Print the resolved config (defaults filled in) as TOML
    Show,
    /// Print the path slurmdash would read from
    Path,
}

#[derive(Debug, Subcommand)]
pub enum DbCommand {
    /// Show database status
    Status,
    /// Apply pending migrations
    Migrate,
    /// VACUUM the database
    Vacuum,
    /// Export data
    Export {
        #[arg(long, value_enum)]
        format: ExportFormat,
    },
    /// Clear cached command output
    ClearCache,
    /// Clear job and resource history
    ClearHistory,
    /// Copy the database to a file
    Backup { path: PathBuf },
}

#[derive(Debug, Clone, ValueEnum)]
pub enum ExportFormat {
    Json,
    Csv,
}

pub async fn dispatch(mut cli: Cli) -> Result<()> {
    let config = crate::config::Config::load(cli.config.as_deref()).context("loading config")?;

    let db = if cli.no_db {
        None
    } else {
        Some(
            crate::db::Db::open(cli.db.clone(), &config)
                .await
                .context("opening local database")?,
        )
    };

    let command = cli.command.take();
    match command {
        Some(Command::Db { cmd }) => handle_db(cmd, db).await,

        Some(Command::Cancel { job_id }) => {
            run_action(
                &cli,
                &config,
                db,
                &job_id,
                crate::actions::ActionKind::Cancel,
            )
            .await
        }
        Some(Command::Hold { job_id }) => {
            run_action(&cli, &config, db, &job_id, crate::actions::ActionKind::Hold).await
        }
        Some(Command::Release { job_id }) => {
            run_action(
                &cli,
                &config,
                db,
                &job_id,
                crate::actions::ActionKind::Release,
            )
            .await
        }
        Some(Command::Requeue { job_id }) => {
            run_action(
                &cli,
                &config,
                db,
                &job_id,
                crate::actions::ActionKind::Requeue,
            )
            .await
        }
        Some(Command::Assist { prompt, job }) => run_assist(&cli, &config, prompt, job).await,
        Some(Command::Config { cmd }) => run_config(cmd, &config, cli.config.as_deref()).await,
        Some(Command::Completions { shell }) => {
            use clap::CommandFactory;
            let mut cmd = Cli::command();
            clap_complete::generate(shell, &mut cmd, "slurmdash", &mut std::io::stdout());
            Ok(())
        }
        Some(Command::Recommend {
            job_name,
            since_days,
        }) => run_recommend(&cli, &config, db, job_name, since_days).await,
        Some(Command::Web {
            host,
            port,
            readonly,
            no_open_browser,
        }) => {
            let handle = crate::ssh::build_runner(&cli, &config).await?;
            let opts = crate::web::WebOptions::from_cli(host, port, readonly, no_open_browser)?;
            crate::web::run(cli, config, handle, db, opts).await
        }
        Some(Command::Logs { .. })
        | Some(Command::Submit { .. })
        | Some(Command::History { .. })
        | Some(Command::Trends) => {
            anyhow::bail!("not yet implemented in Phase 1 MVP")
        }

        Some(Command::Connect { cluster }) => {
            cli.cluster = Some(cluster);
            launch_tui(cli, config, db).await
        }
        None => launch_tui(cli, config, db).await,
    }
}

async fn launch_tui(
    cli: Cli,
    config: crate::config::Config,
    db: Option<crate::db::Db>,
) -> Result<()> {
    let handle = crate::ssh::build_runner(&cli, &config).await?;
    crate::tui::run(cli, config, handle, db).await
}

/// Build a one-paragraph history summary for the selected job's name (if
/// any). Returns None when there's no DB, no job context, or the job has no
/// `job_name` in scontrol output.
async fn build_history_summary(
    cli: &Cli,
    config: &crate::config::Config,
    cluster_name: &str,
    is_local: bool,
    job_context: Option<&crate::assist::JobContext>,
) -> Option<String> {
    if cli.no_db {
        return None;
    }
    let job_name = job_context
        .and_then(|c| c.details.as_ref())
        .and_then(|d| d.job_name.as_deref())?;
    let db = crate::db::Db::open(cli.db.clone(), config).await.ok()?;
    let cluster_id = crate::db::snapshots::ensure_cluster(&db.pool, cluster_name, is_local)
        .await
        .ok()?;
    let stats = crate::history::job_name(&db.pool, cluster_id, job_name, 30)
        .await
        .ok()??;
    Some(crate::history::summarize(&stats))
}

async fn run_recommend(
    cli: &Cli,
    config: &crate::config::Config,
    db: Option<crate::db::Db>,
    job_name: Option<String>,
    since_days: i64,
) -> Result<()> {
    let Some(db) = db else {
        anyhow::bail!("recommend needs the local database (don't pass --no-db)")
    };
    let handle = crate::ssh::build_runner(cli, config).await?;
    let cluster_id =
        crate::db::snapshots::ensure_cluster(&db.pool, &handle.cluster_name, handle.is_local)
            .await?;

    match job_name {
        Some(name) => {
            let stats = crate::history::job_name(&db.pool, cluster_id, &name, since_days).await?;
            if let Some(s) = stats {
                println!("{}", crate::history::summarize(&s));
            } else {
                println!("no snapshots for job name {name:?} in the last {since_days} days");
            }
        }
        None => {
            let all = crate::history::job_name_stats(&db.pool, cluster_id, since_days).await?;
            if all.is_empty() {
                println!(
                    "(no job snapshots in the local database — wait for a few refresh cycles)"
                );
            } else {
                println!(
                    "Job name stats (cluster {}, last {since_days} days):",
                    handle.cluster_name
                );
                for s in all.iter().take(20) {
                    println!("  {}", crate::history::summarize(s));
                }
            }
            println!();
            let parts =
                crate::history::partition_pressure(&db.pool, cluster_id, since_days).await?;
            if !parts.is_empty() {
                println!("Partition pressure (avg pending / running):");
                for p in &parts {
                    println!(
                        "  {:<12}  PD avg {:>5.1}   R avg {:>5.1}   ({} samples)",
                        p.partition, p.avg_pending, p.avg_running, p.samples
                    );
                }
            }
        }
    }
    Ok(())
}

const STARTER_CONFIG: &str = include_str!("../assets/config/starter.toml");

async fn run_config(
    cmd: ConfigCmd,
    config: &crate::config::Config,
    cli_config_path: Option<&std::path::Path>,
) -> Result<()> {
    match cmd {
        ConfigCmd::Init { force, path } => {
            let target = path
                .or_else(|| cli_config_path.map(|p| p.to_path_buf()))
                .or_else(crate::config::default_config_path)
                .context("could not determine config path")?;
            if target.exists() && !force {
                anyhow::bail!(
                    "{} already exists; pass --force to overwrite",
                    target.display()
                );
            }
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("creating {}", parent.display()))?;
            }
            std::fs::write(&target, STARTER_CONFIG)
                .with_context(|| format!("writing {}", target.display()))?;
            println!("wrote {}", target.display());
            Ok(())
        }
        ConfigCmd::Show => {
            let serialized = toml::to_string_pretty(config).context("serializing config")?;
            println!("{serialized}");
            Ok(())
        }
        ConfigCmd::Path => {
            let p = cli_config_path
                .map(|p| p.to_path_buf())
                .or_else(crate::config::default_config_path)
                .context("could not determine config path")?;
            println!("{}", p.display());
            Ok(())
        }
    }
}

async fn run_assist(
    cli: &Cli,
    config: &crate::config::Config,
    prompt: String,
    job: Option<String>,
) -> Result<()> {
    let handle = crate::ssh::build_runner(cli, config).await?;
    let runner = handle.runner.as_ref();

    let jobs_snapshot = crate::slurm::squeue::list(
        runner,
        &crate::slurm::squeue::Options {
            me: false,
            ..Default::default()
        },
    )
    .await
    .unwrap_or_default();
    let partitions = crate::slurm::sinfo::list_partitions(runner)
        .await
        .unwrap_or_default();

    let job_context = match job {
        Some(j) => {
            let details = crate::slurm::scontrol::show(runner, &j).await.ok();
            Some(crate::assist::JobContext { job_id: j, details })
        }
        None => None,
    };

    // If we have a DB, look up history for the selected job's name.
    let history_summary = build_history_summary(
        cli,
        config,
        &handle.cluster_name,
        handle.is_local,
        job_context.as_ref(),
    )
    .await;

    let req = crate::assist::AssistRequest {
        prompt,
        job_context,
        cluster_name: handle.cluster_name.clone(),
        jobs_snapshot,
        partitions,
        history_summary,
    };
    let resp = crate::assist::assist(req, config).await?;

    println!("\n[{} · {}]\n{}\n", resp.provider, resp.model, resp.text);
    if !resp.commands.is_empty() {
        println!("Proposed commands (run with confirmation):");
        for (i, cmd) in resp.commands.iter().enumerate() {
            println!("  {}. {}", i + 1, cmd.preview);
        }
    }
    Ok(())
}

async fn run_action(
    cli: &Cli,
    config: &crate::config::Config,
    db: Option<crate::db::Db>,
    job_id: &str,
    kind: crate::actions::ActionKind,
) -> Result<()> {
    let handle = crate::ssh::build_runner(cli, config).await?;
    crate::actions::run(
        kind,
        job_id,
        handle.runner.as_ref(),
        db.as_ref(),
        &handle.cluster_name,
        handle.is_local,
        true,
    )
    .await?;
    println!("{} {}", kind.label(), job_id);
    Ok(())
}

async fn handle_db(cmd: DbCommand, db: Option<crate::db::Db>) -> Result<()> {
    let Some(db) = db else {
        anyhow::bail!("--no-db was passed; cannot run database subcommands");
    };
    match cmd {
        DbCommand::Status => db.print_status().await,
        DbCommand::Migrate => db.migrate().await,
        DbCommand::Vacuum => db.vacuum().await,
        DbCommand::ClearCache => db.clear_cache().await,
        DbCommand::ClearHistory => db.clear_history().await,
        DbCommand::Backup { path } => db.backup(&path).await,
        DbCommand::Export { format } => match format {
            ExportFormat::Json => db.export_json().await,
            ExportFormat::Csv => db.export_csv().await,
        },
    }
}
