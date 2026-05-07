// Persistent configuration: API keys, wake-word templates, audio device
// preferences, autonomy safeguards. Stored as JSON under the platform's
// standard config directory.

use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub setup_complete: bool,
    /// xAI / Grok API key. Stored in plaintext under the user config dir.
    pub xai_api_key: Option<String>,
    pub xai_model: String,

    pub wake_word_label: String,
    /// Positive (wake-word) feature templates.
    pub wake_templates: Vec<WakeTemplate>,
    /// Negative templates: speech / sounds that should NOT trigger the
    /// wake word. Used to suppress false positives.
    #[serde(default)]
    pub wake_negative_templates: Vec<WakeTemplate>,
    /// Detection threshold for the positive score in `[0, 1]`.
    pub wake_threshold: f32,
    /// Cooldown after a successful detection during which further
    /// triggers are suppressed.
    #[serde(default = "default_cooldown")]
    pub wake_cooldown_secs: u32,

    pub autonomy: Autonomy,

    pub qr_scanning_enabled: bool,
    /// Whether to overlay rectangles around detected QR codes directly
    /// on the user's screen with Open / Copy buttons.
    #[serde(default = "default_true")]
    pub qr_overlay_enabled: bool,

    /// Preferred input device name (cpal). `None` means "system default".
    #[serde(default)]
    pub mic_device: Option<String>,
    /// Preferred output device name. Reserved for future TTS use.
    #[serde(default)]
    pub output_device: Option<String>,
}

fn default_cooldown() -> u32 { 10 }
fn default_true() -> bool { true }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WakeTemplate {
    pub features: Vec<f32>,
    pub frames: usize,
    pub bins: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Autonomy {
    pub allow_app_launch: bool,
    pub allow_input_control: bool,
    pub allow_file_writes: bool,
    pub allow_web_browsing: bool,
    /// Run arbitrary shell commands. High-blast-radius — off by default.
    #[serde(default)]
    pub allow_shell_commands: bool,
    /// Capture screenshots of the user's monitors.
    #[serde(default = "default_true")]
    pub allow_screen_capture: bool,
}

impl Default for Autonomy {
    fn default() -> Self {
        Self {
            allow_app_launch: false,
            allow_input_control: false,
            allow_file_writes: false,
            allow_web_browsing: false,
            allow_shell_commands: false,
            allow_screen_capture: true,
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
            wake_negative_templates: Vec::new(),
            wake_threshold: 0.65,
            wake_cooldown_secs: 10,
            autonomy: Autonomy::default(),
            qr_scanning_enabled: true,
            qr_overlay_enabled: true,
            mic_device: None,
            output_device: None,
        }
    }
}

impl Config {
    pub fn project_dirs() -> Result<ProjectDirs> {
        ProjectDirs::from("com", "OnHands", "Jarvis")
            .context("could not resolve a config directory for this platform")
    }

    pub fn config_dir() -> Result<PathBuf> {
        let dirs = Self::project_dirs()?;
        let dir = dirs.config_dir().to_path_buf();
        std::fs::create_dir_all(&dir).ok();
        Ok(dir)
    }

    pub fn path() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join("config.json"))
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
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, bytes).with_context(|| format!("write {}", tmp.display()))?;
        std::fs::rename(&tmp, &path).with_context(|| format!("rename {}", path.display()))?;
        Ok(())
    }
}
