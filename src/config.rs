use anyhow::{Context, Result, bail};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    pub ui: UiConfig,
    pub database: DatabaseConfig,
    pub clusters: BTreeMap<String, ClusterProfile>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct UiConfig {
    pub theme: String,
    pub refresh_seconds: u64,
    pub mouse: bool,
    pub show_charts: bool,
    pub compact: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: "dark".into(),
            refresh_seconds: 10,
            mouse: true,
            show_charts: true,
            compact: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct DatabaseConfig {
    pub enabled: bool,
    pub engine: String,
    pub path: Option<PathBuf>,
    pub auto_migrate: bool,
    pub retention_days: u32,
    pub cache_ttl_seconds: u64,
    pub store_logs: bool,
    pub store_log_excerpts: bool,
    pub max_log_excerpt_lines: u32,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            engine: "sqlite".into(),
            path: None,
            auto_migrate: true,
            retention_days: 90,
            cache_ttl_seconds: 30,
            store_logs: false,
            store_log_excerpts: true,
            max_log_excerpt_lines: 300,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct ClusterProfile {
    /// If true, skip SSH and run Slurm commands directly on this machine.
    pub local: bool,
    pub host: Option<String>,
    pub user: Option<String>,
    pub port: Option<u16>,
    pub ssh_key: Option<PathBuf>,
    pub default_account: Option<String>,
    pub default_partition: Option<String>,
    pub default_workdir: Option<PathBuf>,
}

impl Config {
    pub fn load(explicit_path: Option<&Path>) -> Result<Self> {
        let path = match explicit_path {
            Some(p) => Some(p.to_path_buf()),
            None => default_config_path(),
        };

        let Some(path) = path else {
            return Ok(Self::default());
        };

        if !path.exists() {
            tracing::debug!(?path, "config file not found; using defaults");
            return Ok(Self::default());
        }

        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let cfg: Self =
            toml::from_str(&raw).with_context(|| format!("parsing {}", path.display()))?;
        Ok(cfg)
    }

    pub fn resolve_cluster(&self, name: Option<&str>) -> Result<ClusterProfile> {
        match name {
            Some(n) => self
                .clusters
                .get(n)
                .cloned()
                .with_context(|| format!("no cluster profile named '{n}'")),
            None => {
                if let Some((_, c)) = self.clusters.iter().next() {
                    return Ok(c.clone());
                }
                bail!("no clusters defined in config and none specified on the command line")
            }
        }
    }
}

pub fn default_config_path() -> Option<PathBuf> {
    ProjectDirs::from("", "", "slurmdash").map(|d| d.config_dir().join("config.toml"))
}

pub fn default_db_path() -> Option<PathBuf> {
    ProjectDirs::from("", "", "slurmdash").map(|d| d.data_dir().join("slurmdash.db"))
}
