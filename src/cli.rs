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
    Connect {
        cluster: String,
    },

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
    let config = crate::config::Config::load(cli.config.as_deref())
        .context("loading config")?;

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
            let runner = crate::ssh::build_runner(&cli, &config).await?;
            crate::slurm::scancel::cancel(runner.as_ref(), &job_id).await?;
            println!("cancelled {job_id}");
            Ok(())
        }
        Some(Command::Hold { job_id }) => {
            let runner = crate::ssh::build_runner(&cli, &config).await?;
            crate::slurm::scontrol::hold(runner.as_ref(), &job_id).await?;
            println!("held {job_id}");
            Ok(())
        }
        Some(Command::Release { job_id }) => {
            let runner = crate::ssh::build_runner(&cli, &config).await?;
            crate::slurm::scontrol::release(runner.as_ref(), &job_id).await?;
            println!("released {job_id}");
            Ok(())
        }
        Some(Command::Requeue { job_id }) => {
            let runner = crate::ssh::build_runner(&cli, &config).await?;
            crate::slurm::scontrol::requeue(runner.as_ref(), &job_id).await?;
            println!("requeued {job_id}");
            Ok(())
        }
        Some(Command::Logs { .. })
        | Some(Command::Submit { .. })
        | Some(Command::History { .. })
        | Some(Command::Trends)
        | Some(Command::Web { .. }) => {
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
    let runner = crate::ssh::build_runner(&cli, &config).await?;
    crate::tui::run(cli, config, runner, db).await
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
