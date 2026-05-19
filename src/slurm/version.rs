use anyhow::Result;

use crate::ssh::Runner;

#[derive(Debug, Clone)]
pub struct SlurmVersion {
    pub raw: String,
}

pub async fn detect(runner: &dyn Runner) -> Result<SlurmVersion> {
    let out = runner.run("squeue", &["--version"]).await?.check("squeue --version")?;
    Ok(SlurmVersion {
        raw: out.stdout.trim().to_string(),
    })
}
