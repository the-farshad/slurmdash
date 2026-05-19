//! Local SQLite database (sqlx).
//!
//! Phase 1 scope:
//! - schema + migrations
//! - settings KV store
//! - cache wrapper (TTL-bounded results)
//! - job/resource snapshot writers (used by the refresh loop)
//! - command audit log
//! - `slurmdash db status` reporting

pub mod audit;
pub mod cache;
pub mod settings;
pub mod snapshots;

use anyhow::{Context, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{ConnectOptions, SqlitePool};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use crate::config::Config;

/// Handle on the local database. Cheap to clone (wraps an Arc'd pool).
#[derive(Clone)]
pub struct Db {
    pub pool: SqlitePool,
    pub path: PathBuf,
}

impl Db {
    pub async fn open(cli_path: Option<PathBuf>, config: &Config) -> Result<Self> {
        let path = cli_path
            .or_else(|| config.database.path.clone())
            .or_else(crate::config::default_db_path)
            .context("could not determine database path")?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }

        let url = format!("sqlite://{}", path.display());
        let options = SqliteConnectOptions::from_str(&url)?
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .foreign_keys(true)
            .log_statements(tracing::log::LevelFilter::Trace);

        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await?;

        let db = Self { pool, path };

        if config.database.auto_migrate {
            db.migrate().await?;
        }

        Ok(db)
    }

    pub async fn migrate(&self) -> Result<()> {
        sqlx::migrate!("./migrations").run(&self.pool).await?;
        Ok(())
    }

    pub async fn vacuum(&self) -> Result<()> {
        sqlx::query("VACUUM").execute(&self.pool).await?;
        println!("ok");
        Ok(())
    }

    pub async fn clear_cache(&self) -> Result<()> {
        sqlx::query("DELETE FROM cache").execute(&self.pool).await?;
        println!("cache cleared");
        Ok(())
    }

    pub async fn clear_history(&self) -> Result<()> {
        sqlx::query("DELETE FROM job_snapshots").execute(&self.pool).await?;
        sqlx::query("DELETE FROM resource_snapshots").execute(&self.pool).await?;
        println!("history cleared");
        Ok(())
    }

    pub async fn backup(&self, dest: &Path) -> Result<()> {
        // Simplest possible: VACUUM INTO copies the database to a new file.
        let sql = format!("VACUUM INTO '{}'", dest.display().to_string().replace('\'', "''"));
        sqlx::query(&sql).execute(&self.pool).await?;
        println!("backup written to {}", dest.display());
        Ok(())
    }

    pub async fn export_json(&self) -> Result<()> {
        anyhow::bail!("db export --format json: deferred to Phase 2")
    }

    pub async fn export_csv(&self) -> Result<()> {
        anyhow::bail!("db export --format csv: deferred to Phase 2")
    }

    pub async fn print_status(&self) -> Result<()> {
        let job_count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM job_snapshots").fetch_one(&self.pool).await?;
        let resource_count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM resource_snapshots").fetch_one(&self.pool).await?;
        let audit_count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM command_audit_log").fetch_one(&self.pool).await?;

        let size = std::fs::metadata(&self.path).map(|m| m.len()).unwrap_or(0);
        println!("Local DB");
        println!("  path:               {}", self.path.display());
        println!("  size:               {} bytes", size);
        println!("  job_snapshots:      {}", job_count.0);
        println!("  resource_snapshots: {}", resource_count.0);
        println!("  audit_log:          {}", audit_count.0);
        Ok(())
    }
}
