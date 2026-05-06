// Minimal client for the xAI (Grok) chat completions API.
//
// xAI exposes an OpenAI-compatible REST endpoint at
// https://api.x.ai/v1/chat/completions, authenticated with a bearer
// token (`xai_api_key` in our config).

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;

const DEFAULT_BASE: &str = "https://api.x.ai/v1";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChatResponse {
    pub choices: Vec<Choice>,
    #[serde(default)]
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Choice {
    pub index: u32,
    pub message: ChatMessage,
    #[serde(default)]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Usage {
    #[serde(default)]
    pub prompt_tokens: u32,
    #[serde(default)]
    pub completion_tokens: u32,
    #[serde(default)]
    pub total_tokens: u32,
}

#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    base: String,
    api_key: String,
}

impl Client {
    pub fn new(api_key: impl Into<String>) -> Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .user_agent("on-hands-jarvis/0.1")
            .build()
            .context("build reqwest client")?;
        Ok(Self {
            http,
            base: DEFAULT_BASE.to_string(),
            api_key: api_key.into(),
        })
    }

    pub fn with_base(mut self, base: impl Into<String>) -> Self {
        self.base = base.into();
        self
    }

    pub async fn chat(&self, req: ChatRequest) -> Result<ChatResponse> {
        if self.api_key.trim().is_empty() {
            return Err(anyhow!("xAI API key is not set"));
        }
        let url = format!("{}/chat/completions", self.base.trim_end_matches('/'));
        let res = self
            .http
            .post(url)
            .bearer_auth(&self.api_key)
            .json(&req)
            .send()
            .await
            .context("send chat completions request")?;

        let status = res.status();
        if !status.is_success() {
            let body = res.text().await.unwrap_or_default();
            return Err(anyhow!("xAI API error {status}: {body}"));
        }
        let parsed: ChatResponse = res
            .json()
            .await
            .context("decode chat completions response")?;
        Ok(parsed)
    }

    pub async fn ask(&self, model: &str, system: Option<&str>, user: &str) -> Result<String> {
        let mut messages = Vec::new();
        if let Some(s) = system {
            messages.push(ChatMessage { role: Role::System, content: s.to_string() });
        }
        messages.push(ChatMessage { role: Role::User, content: user.to_string() });
        let res = self
            .chat(ChatRequest {
                model: model.to_string(),
                messages,
                temperature: Some(0.4),
                max_tokens: Some(800),
            })
            .await?;
        let text = res
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .unwrap_or_default();
        Ok(text)
    }
}
