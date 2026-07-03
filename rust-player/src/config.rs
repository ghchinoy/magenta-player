//! XDG configuration (`config.toml`) plus small path helpers shared across the app.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Configuration Structure for XDG Config Support
///
/// `#[serde(default)]` at the struct level means any field missing from an
/// older config.toml (e.g. after we add a new field in a later version) is
/// filled in from `AppConfig::default()` below, rather than failing to parse
/// -- which would otherwise silently fall through to load_config()'s
/// from-scratch AppConfig::default() + save_config(), wiping out the user's
/// existing saved model path, prompt, etc. Always add new fields with this
/// safety in mind.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct AppConfig {
    pub model: Option<String>,
    pub resources: String,
    pub prompt: String,
    pub temperature: f32,
    pub topk: u32,
    pub midi_gate: bool,
    /// CFG weight for the text/style prompt (MusicCoCa). Higher = more adherent to the prompt.
    pub cfg_text: f32,
    /// CFG weight for MIDI note conditioning.
    pub cfg_notes: f32,
    /// CFG weight for drum conditioning.
    pub cfg_drums: f32,
    /// Suppress drums entirely, independent of the style prompt.
    pub drumless: bool,
    /// Output gain in dB (0.0 = unity gain / no change).
    pub volume_db: f32,
    /// Default directory for --record output WAV files.
    pub output_dir: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            model: None,
            resources: "~/Documents/Magenta/magenta-rt-v2/resources".to_string(),
            prompt: "ambient lofi chords with acoustic guitar".to_string(),
            temperature: 1.3,
            topk: 40,
            midi_gate: false,
            // Match the C++ engine's own factory defaults (mlx_engine.cpp).
            cfg_text: 3.0,
            cfg_notes: 5.0,
            cfg_drums: 1.0,
            drumless: false,
            volume_db: 0.0,
            output_dir: "~/Documents/Magenta/magenta-rt-v2/recordings".to_string(),
        }
    }
}

pub fn get_config_path() -> PathBuf {
    let mut path = dirs::config_dir().expect("❌ Error: Failed to find config directory");
    path.push("magenta-rust-player");
    std::fs::create_dir_all(&path).ok();
    path.push("config.toml");
    path
}

pub fn load_config() -> AppConfig {
    let path = get_config_path();
    if path.exists() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(config) = toml::from_str::<AppConfig>(&content) {
                // Self-heal: if this file predates a field we've since added,
                // #[serde(default)] silently filled it in above -- persist that
                // now so `config list`/the file on disk always reflects the
                // full, effective configuration rather than only what the user
                // has explicitly touched via `config set`.
                save_config(&config);
                return config;
            }
        }
    }
    // Create and write default config if it doesn't exist
    let default_config = AppConfig::default();
    save_config(&default_config);
    default_config
}

pub fn save_config(config: &AppConfig) {
    let path = get_config_path();
    if let Ok(content) = toml::to_string_pretty(config) {
        std::fs::write(&path, content).ok();
    }
}

/// Expands a leading `~/` in a path string to the user's home directory.
pub fn expand_tilde(path: &str) -> String {
    if let Some(stripped) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return format!("{}/{}", home.display(), stripped);
        }
    }
    path.to_string()
}

/// Per mrt2-integration.md: the `resources/` directory (MusicCoCa +
/// SpectroStream assets) is a sibling of the model file somewhere up the
/// tree, not nested inside it, and at an unspecified depth (varies by how
/// the user organized their model downloads) -- so we walk up from the
/// model's parent directory looking for a `resources/` dir rather than
/// hardcoding a level count.
pub fn find_resources_near_model(model_path: &str) -> Option<String> {
    let mut dir = std::path::Path::new(model_path).parent()?.to_path_buf();
    loop {
        let candidate = dir.join("resources");
        if candidate.is_dir() {
            return Some(candidate.to_string_lossy().to_string());
        }
        if !dir.pop() {
            return None;
        }
    }
}
