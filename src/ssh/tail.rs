//! Streaming log tail (placeholder — wired in Phase 1.7 log viewer).
//!
//! The real implementation will spawn `tail -F <path>` on the cluster and
//! stream stdout line-by-line via the openssh crate's command streams.

use anyhow::{Result, bail};

#[allow(dead_code)]
pub async fn tail(_path: &str) -> Result<()> {
    bail!("log tailing not yet implemented")
}
