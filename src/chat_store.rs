// Persistent storage for chat sessions. Each saved chat lives at
// `<config_dir>/chats/<id>.json`. The id is a sortable timestamp, the
// title is auto-derived from the first user message.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::ai::ChatMessage;
use crate::config::Config;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedChat {
    pub id: String,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub messages: Vec<ChatMessage>,
}

#[derive(Debug, Clone)]
pub struct ChatSummary {
    pub id: String,
    pub title: String,
    pub updated_at: DateTime<Utc>,
}

pub fn chats_dir() -> Result<PathBuf> {
    let dir = Config::config_dir()?.join("chats");
    std::fs::create_dir_all(&dir).ok();
    Ok(dir)
}

pub fn new_id() -> String {
    Utc::now().format("%Y%m%dT%H%M%S%3f").to_string()
}

pub fn derive_title(messages: &[ChatMessage]) -> String {
    use crate::ai::Role;
    for m in messages {
        if m.role == Role::User {
            let text: String = m
                .content_str()
                .lines()
                .next()
                .unwrap_or("")
                .chars()
                .take(60)
                .collect();
            if !text.trim().is_empty() {
                return text;
            }
        }
    }
    format!("Chat {}", new_id())
}

pub fn save(chat: &SavedChat) -> Result<()> {
    let path = chats_dir()?.join(format!("{}.json", chat.id));
    let bytes = serde_json::to_vec_pretty(chat)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, bytes).with_context(|| format!("write {}", tmp.display()))?;
    std::fs::rename(&tmp, &path).with_context(|| format!("rename {}", path.display()))?;
    Ok(())
}

pub fn load(id: &str) -> Result<SavedChat> {
    let path = chats_dir()?.join(format!("{id}.json"));
    load_from(&path)
}

pub fn load_from(path: &Path) -> Result<SavedChat> {
    let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let chat: SavedChat = serde_json::from_slice(&bytes)
        .with_context(|| format!("parse {}", path.display()))?;
    Ok(chat)
}

pub fn delete(id: &str) -> Result<()> {
    let path = chats_dir()?.join(format!("{id}.json"));
    if path.exists() {
        std::fs::remove_file(&path).with_context(|| format!("remove {}", path.display()))?;
    }
    Ok(())
}

pub fn list() -> Result<Vec<ChatSummary>> {
    let dir = chats_dir()?;
    let mut out = Vec::new();
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return Ok(out),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        match load_from(&path) {
            Ok(c) => out.push(ChatSummary {
                id: c.id,
                title: c.title,
                updated_at: c.updated_at,
            }),
            Err(e) => log::warn!("skip chat {}: {e}", path.display()),
        }
    }
    out.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(out)
}
