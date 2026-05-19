//! Anthropic (Claude) provider. Opt-in: requires `SLURMDASH_LLM_PROVIDER=anthropic`
//! and `ANTHROPIC_API_KEY`.

use anyhow::{Context, Result, bail};
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};

use super::provider::Provider;
use super::{AssistRequest, AssistResponse, extract_commands, system_prompt};

const ANTHROPIC_VERSION: &str = "2023-06-01";
const DEFAULT_MODEL: &str = "claude-sonnet-4-6";

pub struct AnthropicProvider {
    api_key: String,
    model: String,
    endpoint: String,
}

impl AnthropicProvider {
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("ANTHROPIC_API_KEY").context("ANTHROPIC_API_KEY is not set")?;
        if api_key.trim().is_empty() {
            bail!("ANTHROPIC_API_KEY is empty");
        }
        Ok(Self {
            api_key,
            model: std::env::var("ANTHROPIC_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string()),
            endpoint: std::env::var("ANTHROPIC_ENDPOINT")
                .unwrap_or_else(|_| "https://api.anthropic.com".to_string()),
        })
    }
}

impl Provider for AnthropicProvider {
    fn name(&self) -> &'static str {
        "anthropic"
    }

    fn complete<'a>(&'a self, req: &'a AssistRequest) -> BoxFuture<'a, Result<AssistResponse>> {
        Box::pin(async move {
            let url = format!("{}/v1/messages", self.endpoint.trim_end_matches('/'));
            let body = MessagesRequest {
                model: &self.model,
                max_tokens: 1024,
                system: system_prompt(req),
                messages: vec![ApiMessage {
                    role: "user",
                    content: req.prompt.clone(),
                }],
            };
            let client = reqwest::Client::new();
            let resp = client
                .post(&url)
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", ANTHROPIC_VERSION)
                .json(&body)
                .send()
                .await
                .with_context(|| format!("POST {url}"))?;
            let status = resp.status();
            if !status.is_success() {
                let body = resp.text().await.unwrap_or_default();
                bail!("anthropic returned {status}: {body}");
            }
            let parsed: MessagesResponse =
                resp.json().await.context("decoding anthropic response")?;
            let text = parsed
                .content
                .into_iter()
                .map(|b| match b {
                    ContentBlock::Text { text } => text,
                })
                .collect::<Vec<_>>()
                .join("\n");
            Ok(AssistResponse {
                commands: extract_commands(&text),
                text,
                provider: "anthropic".to_string(),
                model: self.model.clone(),
            })
        })
    }
}

#[derive(Serialize)]
struct MessagesRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    system: String,
    messages: Vec<ApiMessage>,
}

#[derive(Serialize)]
struct ApiMessage {
    role: &'static str,
    content: String,
}

#[derive(Deserialize)]
struct MessagesResponse {
    content: Vec<ContentBlock>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ContentBlock {
    Text { text: String },
}
