//! Simple JSON-valued KV store backed by the `settings` table.
//!
//! Used today for the "last connected host" so users don't have to retype
//! `--host my-alias` on every launch. The schema accepts arbitrary JSON
//! values so future preferences (last sort column, last group_by, etc.)
//! can pile on without migrations.

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct LastConnection {
    pub host: Option<String>,
    pub user: Option<String>,
    pub port: Option<u16>,
    pub ssh_key: Option<String>,
    pub cluster_profile: Option<String>,
}

pub async fn put_last_connection(pool: &sqlx::SqlitePool, value: &LastConnection) -> Result<()> {
    let json = serde_json::to_string(value)?;
    sqlx::query(
        "INSERT INTO settings (key, value_json, updated_at) \
         VALUES ('last_connection', ?, datetime('now')) \
         ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, \
         updated_at = datetime('now')",
    )
    .bind(json)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_last_connection(pool: &sqlx::SqlitePool) -> Result<Option<LastConnection>> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT value_json FROM settings WHERE key = 'last_connection'")
            .fetch_optional(pool)
            .await?;
    match row {
        Some((json,)) => Ok(Some(serde_json::from_str(&json)?)),
        None => Ok(None),
    }
}

pub async fn put_theme(pool: &sqlx::SqlitePool, name: &str) -> Result<()> {
    let json = serde_json::to_string(name)?;
    sqlx::query(
        "INSERT INTO settings (key, value_json, updated_at) \
         VALUES ('theme', ?, datetime('now')) \
         ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, \
         updated_at = datetime('now')",
    )
    .bind(json)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_theme(pool: &sqlx::SqlitePool) -> Result<Option<String>> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT value_json FROM settings WHERE key = 'theme'")
            .fetch_optional(pool)
            .await?;
    match row {
        Some((json,)) => Ok(serde_json::from_str::<String>(&json).ok()),
        None => Ok(None),
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct AssistSettings {
    pub provider: Option<String>, // ollama | anthropic
    pub ollama_host: Option<String>,
    pub ollama_model: Option<String>,
    pub anthropic_model: Option<String>,
}

pub async fn put_assist(pool: &sqlx::SqlitePool, value: &AssistSettings) -> Result<()> {
    let json = serde_json::to_string(value)?;
    sqlx::query(
        "INSERT INTO settings (key, value_json, updated_at) \
         VALUES ('assist', ?, datetime('now')) \
         ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, \
         updated_at = datetime('now')",
    )
    .bind(json)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_assist(pool: &sqlx::SqlitePool) -> Result<Option<AssistSettings>> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT value_json FROM settings WHERE key = 'assist'")
            .fetch_optional(pool)
            .await?;
    match row {
        Some((json,)) => Ok(Some(serde_json::from_str(&json)?)),
        None => Ok(None),
    }
}
