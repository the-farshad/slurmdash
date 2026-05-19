//! Append-only audit log for destructive commands.
//! Phase 1 stub — wired into actions in Phase 1.13.

use anyhow::Result;

#[allow(dead_code)]
pub async fn record(
    _cluster: &str,
    _command_type: &str,
    _command_preview: &str,
    _job_id: Option<&str>,
    _user_confirmed: bool,
    _success: bool,
    _error: Option<&str>,
) -> Result<()> {
    Ok(())
}
