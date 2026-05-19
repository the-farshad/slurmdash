//! Smoke test against the local Slurm CLI binaries (`squeue`, `scontrol`,
//! `scancel`). Only runs when the env var `SLURMDASH_LIVE_SLURM=1` is set,
//! because it requires a reachable `slurmctld`. Useful during local
//! development.

use slurmdash::slurm::squeue;
use slurmdash::ssh::Runner;
use slurmdash::ssh::local::LocalRunner;

fn live() -> bool {
    std::env::var("SLURMDASH_LIVE_SLURM").map(|v| v == "1").unwrap_or(false)
}

#[tokio::test]
async fn squeue_round_trip() {
    if !live() {
        eprintln!("skipped (set SLURMDASH_LIVE_SLURM=1 to run)");
        return;
    }
    let runner = LocalRunner::new();
    let opts = squeue::Options { me: true, ..Default::default() };
    let jobs = squeue::list(&runner as &dyn Runner, &opts).await.unwrap();
    println!("found {} jobs", jobs.len());
}
