//! KV settings stored in the `settings` table (JSON values).
//! Phase 1 stub.

use anyhow::Result;

#[allow(dead_code)]
pub async fn get(_key: &str) -> Result<Option<serde_json::Value>> {
    Ok(None)
}

#[allow(dead_code)]
pub async fn put(_key: &str, _value: &serde_json::Value) -> Result<()> {
    Ok(())
}
