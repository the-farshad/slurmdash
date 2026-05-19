use anyhow::{Context, Result, bail};
use futures::future::BoxFuture;

use super::{AssistRequest, AssistResponse};
use crate::config::Config;

pub trait Provider: Send + Sync {
    fn name(&self) -> &'static str;
    fn complete<'a>(&'a self, req: &'a AssistRequest) -> BoxFuture<'a, Result<AssistResponse>>;
}

/// Resolve the configured provider. Default is Ollama (local). Override
/// via `[assist].provider = "anthropic"` in config or the
/// `SLURMDASH_LLM_PROVIDER` env var.
pub fn resolve(config: &Config) -> Result<Box<dyn Provider>> {
    let kind = std::env::var("SLURMDASH_LLM_PROVIDER")
        .ok()
        .or_else(|| config_provider(config))
        .unwrap_or_else(|| "ollama".to_string());

    match kind.as_str() {
        "ollama" => Ok(Box::new(super::ollama::OllamaProvider::from_env())),
        "anthropic" => super::anthropic::AnthropicProvider::from_env()
            .map(|p| Box::new(p) as Box<dyn Provider>)
            .context("setting up Anthropic provider"),
        other => bail!("unknown LLM provider {other:?} (try ollama or anthropic)"),
    }
}

fn config_provider(_config: &Config) -> Option<String> {
    // Phase 4 stub: the [assist] config table will land in a follow-up.
    None
}
