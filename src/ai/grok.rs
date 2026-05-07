// Minimal client for the xAI (Grok) chat completions API. xAI exposes an
// OpenAI-compatible REST endpoint at https://api.x.ai/v1/chat/completions
// authenticated with a bearer token (the `xai_api_key` in our config).
//
// This module models tool / function calling so an agent loop in the
// chat view can let Grok invoke local tools (system info, QR codes,
// app launch, etc.) between user turns.

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
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    /// Assistant messages with `tool_calls` may legitimately have `None`
    /// content; tool result messages always carry their stringified
    /// payload here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Set only on assistant messages that requested tool calls.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    /// Set only on `Role::Tool` messages: the id of the call this
    /// message is responding to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// Optional friendly name for the tool that produced this message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl ChatMessage {
    pub fn system(text: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: Some(text.into()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: Some(text.into()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }
    pub fn assistant(text: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: Some(text.into()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }
    pub fn tool_result(call_id: impl Into<String>, name: impl Into<String>, payload: impl Into<String>) -> Self {
        Self {
            role: Role::Tool,
            content: Some(payload.into()),
            tool_calls: None,
            tool_call_id: Some(call_id.into()),
            name: Some(name.into()),
        }
    }

    pub fn content_str(&self) -> &str {
        self.content.as_deref().unwrap_or("")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String, // "function"
    pub function: FunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    /// JSON-encoded arguments string, per the OpenAI / xAI spec.
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolDef {
    #[serde(rename = "type")]
    pub kind: &'static str, // "function"
    pub function: FunctionDef,
}

#[derive(Debug, Clone, Serialize)]
pub struct FunctionDef {
    pub name: String,
    pub description: String,
    /// JSON-Schema describing the tool's argument object.
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolDef>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<String>,
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
}
