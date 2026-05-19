//! Local Ollama provider — the default. Talks to `http://localhost:11434`
//! by default; override the host with `OLLAMA_HOST` and the model with
//! `OLLAMA_MODEL`.

use anyhow::{Context, Result};
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};

use super::provider::Provider;
use super::{AssistRequest, AssistResponse, extract_commands, system_prompt};

pub struct OllamaProvider {
    host: String,
    model: String,
}

impl OllamaProvider {
    pub fn from_env() -> Self {
        let raw = std::env::var("OLLAMA_HOST")
            .unwrap_or_else(|_| "http://localhost:11434".to_string());
        // Tolerate the common `OLLAMA_HOST=host:port` form (Ollama itself
        // accepts that without a scheme) by adding the missing scheme.
        let host = if raw.starts_with("http://") || raw.starts_with("https://") {
            raw
        } else {
            format!("http://{raw}")
        };
        Self {
            host,
            model: std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "llama3.2".to_string()),
        }
    }
}

impl Provider for OllamaProvider {
    fn name(&self) -> &'static str {
        "ollama"
    }

    fn complete<'a>(
        &'a self,
        req: &'a AssistRequest,
    ) -> BoxFuture<'a, Result<AssistResponse>> {
        Box::pin(async move {
            let url = format!("{}/api/chat", self.host.trim_end_matches('/'));
            let body = ChatRequest {
                model: &self.model,
                stream: false,
                messages: vec![
                    Message { role: "system", content: system_prompt(req) },
                    Message { role: "user", content: req.prompt.clone() },
                ],
            };
            let client = reqwest::Client::new();
            let resp = client
                .post(&url)
                .json(&body)
                .send()
                .await
                .with_context(|| format!("POST {url}"))?;
            let status = resp.status();
            if !status.is_success() {
                let body = resp.text().await.unwrap_or_default();
                anyhow::bail!("ollama returned {status}: {body}");
            }
            let parsed: ChatResponse = resp.json().await.context("decoding ollama response")?;
            let text = parsed.message.content;
            Ok(AssistResponse {
                commands: extract_commands(&text),
                text,
                provider: "ollama".to_string(),
                model: self.model.clone(),
            })
        })
    }
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    stream: bool,
    messages: Vec<Message>,
}

#[derive(Serialize, Deserialize)]
struct Message {
    role: &'static str,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    message: ChatMessage,
}

#[derive(Deserialize)]
struct ChatMessage {
    #[allow(dead_code)]
    role: String,
    content: String,
}
