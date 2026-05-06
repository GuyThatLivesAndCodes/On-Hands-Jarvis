// Persistent configuration: API keys, wake-word templates, autonomy
// safeguards. Stored as JSON under the platform's standard config directory.
//
// `Config` is the on-disk schema; mutate, then call `save` to persist.

use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub setup_complete: bool,
    /// Optional Grok / xAI API key. Stored in plaintext in the user config
    /// dir; treat with normal config-file hygiene.
    pub xai_api_key: Option<String>,
    /// Model to call against the xAI Chat API.
    pub xai_model: String,
    /// User's chosen wake word (free-form text label only).
    pub wake_word_label: String,
    /// Wake-word feature templates (one per recorded sample).
    pub wake_templates: Vec<WakeTemplate>,
    /// Detection sensitivity: lower = more permissive, higher = stricter.
    pub wake_threshold: f32,
    /// Autonomy safeguards.
    pub autonomy: Autonomy,
    /// QR code passive scanning of the screen.
    pub qr_scanning_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WakeTemplate {
    /// Flat MFCC-like log-spectrogram features.
    pub features: Vec<f32>,
    /// Number of frames (rows) in the original 2D feature matrix.
    pub frames: usize,
    /// Number of bins per frame (columns).
    pub bins: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Autonomy {
    pub allow_app_launch: bool,
    pub allow_input_control: bool,
    pub allow_file_writes: bool,
    pub allow_web_browsing: bool,
}

impl Default for Autonomy {
    fn default() -> Self {
        Self {
            allow_app_launch: false,
            allow_input_control: false,
            allow_file_writes: false,
            allow_web_browsing: false,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            setup_complete: false,
            xai_api_key: None,
            xai_model: "grok-2-latest".to_string(),
            wake_word_label: "Jarvis".to_string(),
            wake_templates: Vec::new(),
            wake_threshold: 0.65,
            autonomy: Autonomy::default(),
            qr_scanning_enabled: true,
        }
    }
}

impl Config {
    pub fn path() -> Result<PathBuf> {
        let dirs = ProjectDirs::from("com", "OnHands", "Jarvis")
            .context("could not resolve a config directory for this platform")?;
        let dir = dirs.config_dir().to_path_buf();
        std::fs::create_dir_all(&dir).ok();
        Ok(dir.join("config.json"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::path()?;
        Self::load_from(&path)
    }

    pub fn load_from(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
        let cfg: Config = serde_json::from_slice(&bytes)
            .with_context(|| format!("parse {}", path.display()))?;
        Ok(cfg)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path()?;
        let bytes = serde_json::to_vec_pretty(self)?;
        // Write to a tempfile then rename, so a crash doesn't truncate the
        // user's config.
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, bytes).with_context(|| format!("write {}", tmp.display()))?;
        std::fs::rename(&tmp, &path).with_context(|| format!("rename {}", path.display()))?;
        Ok(())
    }
}
