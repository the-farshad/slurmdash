//! TTL-bounded cache for short-lived Slurm command output.
//! Phase 1 stub — wired up alongside sinfo/qos calls in Phase 2.

use anyhow::Result;

#[allow(dead_code)]
pub async fn get(_key: &str) -> Result<Option<String>> {
    Ok(None)
}

#[allow(dead_code)]
pub async fn put(_key: &str, _value: &str, _ttl_seconds: u64) -> Result<()> {
    Ok(())
}
