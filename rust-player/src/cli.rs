//! Command-line interface: clap arg structs and the `config` subcommand handler.

use crate::config::{get_config_path, save_config, AppConfig};
use clap::{Args as ClapArgs, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "magenta-rust-player", author, version, about = "Rust CLI Player for Magenta RealTime 2", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Start real-time audio playback (CLI overrides local XDG config defaults)
    Play(PlayArgs),

    /// View or modify the local XDG config settings
    Config(ConfigArgs),
}

#[derive(ClapArgs, Debug, Default)]
pub struct PlayArgs {
    /// Path to the model directory or .mlxfn file
    #[arg(short = 'm', long, value_name = "MODEL_PATH")]
    pub model: Option<PathBuf>,

    /// Path to the assets/resources directory
    #[arg(short = 'r', long, value_name = "RESOURCES_PATH")]
    pub resources: Option<String>,

    /// Text style conditioning prompt
    #[arg(short = 'p', long)]
    pub prompt: Option<String>,

    /// Generation temperature (scales randomness)
    #[arg(short = 't', long)]
    pub temperature: Option<f32>,

    /// Top-K sampling (restricts likely choices)
    #[arg(short = 'k', long)]
    pub topk: Option<u32>,

    /// Enable low-latency MIDI gate envelope
    #[arg(short = 'g', long)]
    pub midi_gate: Option<bool>,

    /// CFG weight for the text/style prompt (higher = more adherent to prompt). Factory default: 3.0
    #[arg(long)]
    pub cfg_text: Option<f32>,

    /// CFG weight for MIDI note conditioning. Factory default: 5.0
    #[arg(long)]
    pub cfg_notes: Option<f32>,

    /// CFG weight for drum conditioning. Factory default: 1.0
    #[arg(long)]
    pub cfg_drums: Option<f32>,

    /// Suppress drums entirely, independent of the style prompt
    #[arg(long)]
    pub drumless: Option<bool>,

    /// Output gain in dB (0.0 = unity gain). Use --volume-db=-6.0 syntax for negative values.
    #[arg(long, allow_hyphen_values = true)]
    pub volume_db: Option<f32>,

    /// Record a WAV clip of this session and exit once done (see --record-seconds, --output-dir)
    #[arg(long)]
    pub record: bool,

    /// Duration in seconds to record when --record is set
    #[arg(long, default_value_t = 10)]
    pub record_seconds: u64,

    /// Directory to save recorded WAV clips into (overrides config output_dir)
    #[arg(long, value_name = "OUTPUT_DIR")]
    pub output_dir: Option<String>,
}

#[derive(ClapArgs, Debug)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub action: ConfigAction,
}

#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// Show the path and contents of the configuration file
    List,

    /// Print the absolute file path to config.toml
    Path,

    /// Modify a specific configuration value (e.g. config set prompt "jazz")
    Set {
        /// Configuration key to modify (model, resources, prompt, temperature, topk, midi_gate,
        /// cfg_text, cfg_notes, cfg_drums, drumless, volume_db)
        key: String,

        /// New value for the configuration key (negative numbers OK, e.g. -6.0)
        #[arg(allow_hyphen_values = true)]
        value: String,
    },
}

/// Handle the `config` subcommand (list / path / set).
pub fn handle_config(mut config: AppConfig, action: ConfigAction) {
    match action {
        ConfigAction::Path => {
            println!("{}", get_config_path().display());
        }
        ConfigAction::List => {
            let path = get_config_path();
            println!("Config File Path: {}\n", path.display());
            if let Ok(content) = std::fs::read_to_string(&path) {
                println!("{}", content);
            }
        }
        ConfigAction::Set { key, value } => {
            match key.as_str() {
                "model" => {
                    config.model = if value.is_empty() || value == "none" {
                        None
                    } else {
                        Some(value.clone())
                    };
                }
                "resources" => config.resources = value.clone(),
                "prompt" => config.prompt = value.clone(),
                "temperature" => config.temperature = parse_or_exit(&value, "temperature (float)"),
                "topk" => config.topk = parse_or_exit(&value, "topk (integer)"),
                "midi_gate" => config.midi_gate = parse_or_exit(&value, "midi_gate (true/false)"),
                "cfg_text" => config.cfg_text = parse_or_exit(&value, "cfg_text (float)"),
                "cfg_notes" => config.cfg_notes = parse_or_exit(&value, "cfg_notes (float)"),
                "cfg_drums" => config.cfg_drums = parse_or_exit(&value, "cfg_drums (float)"),
                "drumless" => config.drumless = parse_or_exit(&value, "drumless (true/false)"),
                "volume_db" => config.volume_db = parse_or_exit(&value, "volume_db (float)"),
                "output_dir" => config.output_dir = value.clone(),
                _ => {
                    eprintln!("❌ Error: Unknown configuration key '{}'", key);
                    eprintln!("Valid keys: model, resources, prompt, temperature, topk, midi_gate, cfg_text, cfg_notes, cfg_drums, drumless, volume_db, output_dir");
                    std::process::exit(1);
                }
            }
            save_config(&config);
            println!("✓ Successfully set '{}' to '{}' in config!", key, value);
        }
    }
}

/// Parse a config value or exit(1) with a descriptive error.
fn parse_or_exit<T: std::str::FromStr>(value: &str, what: &str) -> T {
    value.parse::<T>().unwrap_or_else(|_| {
        eprintln!("❌ Error: Failed to parse {} value: '{}'", what, value);
        std::process::exit(1);
    })
}
