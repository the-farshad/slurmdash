use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    slurmdash::run().await
}
