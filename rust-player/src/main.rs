use clap::{Args as ClapArgs, Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph, Sparkline, Wrap},
    Terminal,
};

// Bidirectional safe FFI bridge using cxx
#[cxx::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("magenta-rust-player/src/bridge.h");

        // We can expose the C++ RealtimeRunner class directly to Rust
        type RealtimeRunnerBridge;

        fn create_runner() -> UniquePtr<RealtimeRunnerBridge>;
        fn init_assets(self: Pin<&mut RealtimeRunnerBridge>, resource_dir: &str) -> bool;
        fn load_model(self: Pin<&mut RealtimeRunnerBridge>, path: &str) -> bool;
        fn set_prompt(self: Pin<&mut RealtimeRunnerBridge>, prompt: &str);
        fn set_temperature(self: Pin<&mut RealtimeRunnerBridge>, temp: f32);
        fn set_top_k(self: Pin<&mut RealtimeRunnerBridge>, k: u32);
        fn set_midi_gate(self: Pin<&mut RealtimeRunnerBridge>, enabled: bool);
        fn set_buffer_size(self: Pin<&mut RealtimeRunnerBridge>, cap: usize);
        fn set_cfg_text(self: Pin<&mut RealtimeRunnerBridge>, v: f32);
        fn set_cfg_notes(self: Pin<&mut RealtimeRunnerBridge>, v: f32);
        fn set_cfg_drums(self: Pin<&mut RealtimeRunnerBridge>, v: f32);
        fn set_drumless(self: Pin<&mut RealtimeRunnerBridge>, on: bool);
        fn set_volume_db(self: Pin<&mut RealtimeRunnerBridge>, v: f32);
        fn toggle_play(self: &RealtimeRunnerBridge, playing: bool);
        fn get_quantizer_status(self: &RealtimeRunnerBridge) -> i32;
        fn read_audio_stereo(
            self: &RealtimeRunnerBridge,
            dest_l: &mut [f32],
            dest_r: &mut [f32],
        ) -> bool;
        fn read_metrics(self: &RealtimeRunnerBridge) -> String;
        fn start_recording(self: &RealtimeRunnerBridge);
        fn stop_recording(self: &RealtimeRunnerBridge);
        fn get_recorded_sample_count(self: &RealtimeRunnerBridge) -> usize;
        fn get_recorded_audio(
            self: &RealtimeRunnerBridge,
            start_idx: usize,
            dest_l: &mut [f32],
            dest_r: &mut [f32],
        ) -> bool;
        fn reset_dropped_frames(self: &RealtimeRunnerBridge);
    }
}

// Implement Send and Sync so we can share the runner with the cpal audio callback thread safely.
unsafe impl Send for ffi::RealtimeRunnerBridge {}
unsafe impl Sync for ffi::RealtimeRunnerBridge {}

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
struct AppConfig {
    model: Option<String>,
    resources: String,
    prompt: String,
    temperature: f32,
    topk: u32,
    midi_gate: bool,
    /// CFG weight for the text/style prompt (MusicCoCa). Higher = more adherent to the prompt.
    cfg_text: f32,
    /// CFG weight for MIDI note conditioning.
    cfg_notes: f32,
    /// CFG weight for drum conditioning.
    cfg_drums: f32,
    /// Suppress drums entirely, independent of the style prompt.
    drumless: bool,
    /// Output gain in dB (0.0 = unity gain / no change).
    volume_db: f32,
    /// Default directory for --record output WAV files.
    output_dir: String,
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

fn get_config_path() -> PathBuf {
    let mut path = dirs::config_dir().expect("❌ Error: Failed to find config directory");
    path.push("magenta-rust-player");
    std::fs::create_dir_all(&path).ok();
    path.push("config.toml");
    path
}

fn load_config() -> AppConfig {
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

fn save_config(config: &AppConfig) {
    let path = get_config_path();
    if let Ok(content) = toml::to_string_pretty(config) {
        std::fs::write(&path, content).ok();
    }
}

#[derive(Parser, Debug)]
#[command(name = "magenta-rust-player", author, version, about = "Rust CLI Player for Magenta RealTime 2", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Start real-time audio playback (CLI overrides local XDG config defaults)
    Play(PlayArgs),
    
    /// View or modify the local XDG config settings
    Config(ConfigArgs),
}

#[derive(ClapArgs, Debug, Default)]
struct PlayArgs {
    /// Path to the model directory or .mlxfn file
    #[arg(short = 'm', long, value_name = "MODEL_PATH")]
    model: Option<PathBuf>,

    /// Path to the assets/resources directory
    #[arg(short = 'r', long, value_name = "RESOURCES_PATH")]
    resources: Option<String>,

    /// Text style conditioning prompt
    #[arg(short = 'p', long)]
    prompt: Option<String>,

    /// Generation temperature (scales randomness)
    #[arg(short = 't', long)]
    temperature: Option<f32>,

    /// Top-K sampling (restricts likely choices)
    #[arg(short = 'k', long)]
    topk: Option<u32>,

    /// Enable low-latency MIDI gate envelope
    #[arg(short = 'g', long)]
    midi_gate: Option<bool>,

    /// CFG weight for the text/style prompt (higher = more adherent to prompt). Factory default: 3.0
    #[arg(long)]
    cfg_text: Option<f32>,

    /// CFG weight for MIDI note conditioning. Factory default: 5.0
    #[arg(long)]
    cfg_notes: Option<f32>,

    /// CFG weight for drum conditioning. Factory default: 1.0
    #[arg(long)]
    cfg_drums: Option<f32>,

    /// Suppress drums entirely, independent of the style prompt
    #[arg(long)]
    drumless: Option<bool>,

    /// Output gain in dB (0.0 = unity gain). Use --volume-db=-6.0 syntax for negative values.
    #[arg(long, allow_hyphen_values = true)]
    volume_db: Option<f32>,

    /// Record a WAV clip of this session and exit once done (see --record-seconds, --output-dir)
    #[arg(long)]
    record: bool,

    /// Duration in seconds to record when --record is set
    #[arg(long, default_value_t = 10)]
    record_seconds: u64,

    /// Directory to save recorded WAV clips into (overrides config output_dir)
    #[arg(long, value_name = "OUTPUT_DIR")]
    output_dir: Option<String>,
}

#[derive(ClapArgs, Debug)]
struct ConfigArgs {
    #[command(subcommand)]
    action: ConfigAction,
}

#[derive(Subcommand, Debug)]
enum ConfigAction {
    /// Show the path and contents of the configuration file
    List,
    
    /// Print the absolute file path to config.toml
    Path,
    
    /// Modify a specific configuration value (e.g. config set prompt \"jazz\")
    Set {
        /// Configuration key to modify (model, resources, prompt, temperature, topk, midi_gate,
        /// cfg_text, cfg_notes, cfg_drums, drumless, volume_db)
        key: String,
        
        /// New value for the configuration key (negative numbers OK, e.g. -6.0)
        #[arg(allow_hyphen_values = true)]
        value: String,
    },
}

fn main() {
    // Initialize standard environment logger
    env_logger::init();
    
    // Load config from standard XDG path
    let mut config = load_config();

    // Parse CLI arguments
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Config(cfg_args)) => {
            match cfg_args.action {
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
                            config.model = if value.is_empty() || value == "none" { None } else { Some(value.clone()) };
                        }
                        "resources" => {
                            config.resources = value.clone();
                        }
                        "prompt" => {
                            config.prompt = value.clone();
                        }
                        "temperature" => {
                            if let Ok(val) = value.parse::<f32>() {
                                config.temperature = val;
                            } else {
                                eprintln!("❌ Error: Failed to parse temperature as float");
                                std::process::exit(1);
                            }
                        }
                        "topk" => {
                            if let Ok(val) = value.parse::<u32>() {
                                config.topk = val;
                            } else {
                                eprintln!("❌ Error: Failed to parse topk as integer");
                                std::process::exit(1);
                            }
                        }
                        "midi_gate" => {
                            if let Ok(val) = value.parse::<bool>() {
                                config.midi_gate = val;
                            } else {
                                eprintln!("❌ Error: Failed to parse midi_gate as boolean (true/false)");
                                std::process::exit(1);
                            }
                        }
                        "cfg_text" => {
                            if let Ok(val) = value.parse::<f32>() {
                                config.cfg_text = val;
                            } else {
                                eprintln!("❌ Error: Failed to parse cfg_text as float");
                                std::process::exit(1);
                            }
                        }
                        "cfg_notes" => {
                            if let Ok(val) = value.parse::<f32>() {
                                config.cfg_notes = val;
                            } else {
                                eprintln!("❌ Error: Failed to parse cfg_notes as float");
                                std::process::exit(1);
                            }
                        }
                        "cfg_drums" => {
                            if let Ok(val) = value.parse::<f32>() {
                                config.cfg_drums = val;
                            } else {
                                eprintln!("❌ Error: Failed to parse cfg_drums as float");
                                std::process::exit(1);
                            }
                        }
                        "drumless" => {
                            if let Ok(val) = value.parse::<bool>() {
                                config.drumless = val;
                            } else {
                                eprintln!("❌ Error: Failed to parse drumless as boolean (true/false)");
                                std::process::exit(1);
                            }
                        }
                        "volume_db" => {
                            if let Ok(val) = value.parse::<f32>() {
                                config.volume_db = val;
                            } else {
                                eprintln!("❌ Error: Failed to parse volume_db as float");
                                std::process::exit(1);
                            }
                        }
                        "output_dir" => {
                            config.output_dir = value.clone();
                        }
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
        Some(Commands::Play(play_args)) => {
            run_player(config, play_args);
        }
        None => {
            // Default to play with default arguments if no subcommand is supplied
            run_player(config, PlayArgs::default());
        }
    }
}

/// Expands a leading `~/` in a path string to the user's home directory.
fn expand_tilde(path: &str) -> String {
    if let Some(stripped) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return format!("{}/{}", home.display(), stripped);
        }
    }
    path.to_string()
}

/// Creates a consistently-styled animated spinner for blocking/async loading
/// phases (MusicCoCa asset init, model load, prompt encoding). Uses
/// enable_steady_tick, which spawns its own background redraw thread -- this
/// animates correctly even while the calling thread is blocked in a
/// synchronous FFI call (e.g. load_model compiling the MLX graph).
fn new_spinner(msg: impl Into<String>) -> indicatif::ProgressBar {
    let pb = indicatif::ProgressBar::new_spinner();
    pb.set_style(
        indicatif::ProgressStyle::default_spinner()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ ")
            .template("{spinner:.cyan} {msg}")
            .unwrap(),
    );
    pb.set_message(msg.into());
    pb.enable_steady_tick(std::time::Duration::from_millis(80));
    pb
}

/// Per mrt2-integration.md: the `resources/` directory (MusicCoCa +
/// SpectroStream assets) is a sibling of the model file somewhere up the
/// tree, not nested inside it, and at an unspecified depth (varies by how
/// the user organized their model downloads) -- so we walk up from the
/// model's parent directory looking for a `resources/` dir rather than
/// hardcoding a level count.
fn find_resources_near_model(model_path: &str) -> Option<String> {
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

fn run_player(config: AppConfig, args: PlayArgs) {
    // Merge logic: CLI arguments override XDG config file defaults
    let model_path = args.model.map(|p| p.to_string_lossy().to_string()).or(config.model);
    let mut resources_path = expand_tilde(&args.resources.unwrap_or(config.resources));
    // If the configured/default resources path doesn't actually exist, try to
    // auto-derive it from the model's location before giving up -- helps
    // anyone whose models live somewhere other than the standard
    // magenta-rt-v2/ layout without requiring them to configure --resources
    // by hand. Only used as a fallback: an explicitly-set path that exists
    // always wins.
    if !std::path::Path::new(&resources_path).is_dir() {
        if let Some(ref model) = model_path {
            if let Some(found) = find_resources_near_model(&expand_tilde(model)) {
                println!("[INFO] Configured resources path not found ({}); auto-derived from model location: {}", resources_path, found);
                resources_path = found;
            }
        }
    }
    let prompt = args.prompt.unwrap_or(config.prompt);
    let temperature = args.temperature.unwrap_or(config.temperature);
    let topk = args.topk.unwrap_or(config.topk);
    let midi_gate = args.midi_gate.unwrap_or(config.midi_gate);
    let cfg_text = args.cfg_text.unwrap_or(config.cfg_text);
    let cfg_notes = args.cfg_notes.unwrap_or(config.cfg_notes);
    let cfg_drums = args.cfg_drums.unwrap_or(config.cfg_drums);
    let drumless = args.drumless.unwrap_or(config.drumless);
    let volume_db = args.volume_db.unwrap_or(config.volume_db);
    let output_dir = expand_tilde(&args.output_dir.unwrap_or(config.output_dir));
    let record = args.record;
    let record_seconds = args.record_seconds;

    println!("=== Magenta RealTime 2 Rust Player CLI ===");
    println!("Prompt:      \"{}\"", prompt);
    println!("Temperature: {}", temperature);
    println!("Top-K:       {}", topk);
    println!("MIDI Gate:   {}", if midi_gate { "Enabled" } else { "Disabled" });
    println!("CFG (text/notes/drums): {:.1} / {:.1} / {:.1}", cfg_text, cfg_notes, cfg_drums);
    println!("Drumless:    {}", drumless);
    println!("Volume:      {:.1} dB", volume_db);
    println!("Resources:   {}", resources_path);
    if let Some(ref path) = model_path {
        println!("Model Path:  {}", path);
    } else {
        println!("Model Path:  None");
    }
    if record {
        println!("Recording:   {} seconds -> {}", record_seconds, output_dir);
    }
    println!("=========================================");

    // 1. Initialize the C++ RealtimeRunner via the cxx bridge:
    println!("\nInitializing C++ RealtimeRunner...");
    let mut runner_unique = ffi::create_runner();

    // 2. Load the MusicCoCa tokenizer/text-encoder/quantizer assets.
    //    REQUIRED before set_prompt() can have any effect — without this,
    //    the engine silently fails to encode prompts and stays on its
    //    hardcoded default (piano) tokens forever.
    println!("Loading MusicCoCa assets from: {}", resources_path);
    let assets_spinner = new_spinner("Loading...");
    let assets_ok = runner_unique.pin_mut().init_assets(&resources_path);
    // finish_and_clear() (not finish_with_message()) because indicatif silently
    // suppresses ALL drawing -- including the finish message -- when stderr
    // isn't a TTY (piped output, log files, CI). We always want this status
    // line visible, so we print it ourselves after clearing the spinner:
    // animated spinner only when interactive, guaranteed status everywhere.
    assets_spinner.finish_and_clear();
    if assets_ok {
        println!("✓ MusicCoCa assets (tokenizer/text-encoder/quantizer) loaded!");
    } else {
        eprintln!("❌ Error: Failed to load MusicCoCa assets from {}", resources_path);
        eprintln!("         Prompts will NOT take effect; the engine will stay on its default tokens.");
        eprintln!("         Check --resources / `config set resources <path>` points at magenta-rt-v2/resources.");
    }

    // 3. Set the initial generation parameters (now that assets are loaded, set_prompt can actually encode):
    runner_unique.pin_mut().set_prompt(&prompt);
    runner_unique.pin_mut().set_temperature(temperature);
    runner_unique.pin_mut().set_top_k(topk);
    runner_unique.pin_mut().set_midi_gate(midi_gate);
    runner_unique.pin_mut().set_cfg_text(cfg_text);
    runner_unique.pin_mut().set_cfg_notes(cfg_notes);
    runner_unique.pin_mut().set_cfg_drums(cfg_drums);
    runner_unique.pin_mut().set_drumless(drumless);
    runner_unique.pin_mut().set_volume_db(volume_db);
    
    // Set ring buffer virtual capacity to 8192 samples (RingBuffer::kCapacity, the
    // physical maximum, ~170ms/4 frames at 48kHz). docs/realtime-audio.md and
    // mrt2-prompt-and-drift.md both call out that 4096 (~85ms) still allows
    // occasional underruns on mrt2_base due to Metal scheduling jitter (50-80ms/frame
    // observed on borderline hardware); 8192 absorbs that variance. This only fixes
    // *jitter*-caused drops -- if dropped_frames still grows unbounded, that's a
    // genuine hardware throughput limit no buffer size can fix (switch to mrt2_small).
    println!("Configuring C++ ring buffer size to 8192 samples (max, ~170ms headroom)...");
    runner_unique.pin_mut().set_buffer_size(8192);

    // 4. Load the model if provided:
    if let Some(ref path_str) = model_path {
        let expanded_path = expand_tilde(path_str);
        // load_model compiles the MLX computation graph and can take 5-15s on
        // first load -- the spinner's steady tick keeps animating throughout
        // even though this call blocks the current thread.
        println!("Loading model from: {} (this can take 5-15s on first load)", expanded_path);
        let model_spinner = new_spinner("Compiling MLX graph...");
        let load_ok = runner_unique.pin_mut().load_model(&expanded_path);
        model_spinner.finish_and_clear();
        if load_ok {
            println!("✓ Model loaded successfully!");
        } else {
            eprintln!("❌ Error: Failed to load model from {}", expanded_path);
            std::process::exit(1);
        }
    } else {
        println!("[WARNING] No model path specified. Use play --model <PATH> or config set model <PATH> to load.");
    }

    // 4b. Wait for the async MusicCoCa prompt encode (tokenize -> text-encoder ->
    //     mapper -> quantize) to finish before unmuting audio, so we never play
    //     back the engine's hardcoded default (piano) tokens.
    //     Status codes: 0=idle, 1=fetching, 2=success, 3=error.
    println!("Encoding style prompt...");
    let encode_spinner = new_spinner("Waiting for encoder...");
    let encode_timeout = std::time::Duration::from_secs(5);
    let encode_start = std::time::Instant::now();
    loop {
        let status = runner_unique.get_quantizer_status();
        if status == 2 {
            encode_spinner.finish_and_clear();
            println!("✓ Style prompt encoded!");
            break;
        }
        if status == 3 {
            encode_spinner.finish_and_clear();
            eprintln!("[WARNING] Prompt encoding failed (status=3). Check that --resources points at a valid magenta-rt-v2/resources directory.");
            break;
        }
        if encode_start.elapsed() > encode_timeout {
            encode_spinner.finish_and_clear();
            eprintln!("[WARNING] Timed out waiting for prompt encoding after {:?}; starting anyway.", encode_timeout);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(30));
    }

    // 5. Wrap runner in Arc to share with the cpal audio thread
    let runner = Arc::new(runner_unique);

    // 5. Initialize the default audio output device using cpal
    println!("Opening default audio output device...");
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .expect("❌ Error: No audio output device available");
    
    // Query supported configurations to request exactly 48000 Hz stereo
    let supported_configs_range = device
        .supported_output_configs()
        .expect("❌ Error: Failed to query supported audio configurations");
        
    let config_format = supported_configs_range
        .filter(|c| c.channels() == 2)
        .find(|c| c.min_sample_rate().0 <= 48000 && c.max_sample_rate().0 >= 48000)
        .map(|c| c.with_sample_rate(cpal::SampleRate(48000)))
        .unwrap_or_else(|| {
            let default_config = device.default_output_config().expect("❌ Error: Failed to get default audio output configuration");
            println!("\n[WARNING] 48kHz stereo output not directly supported by this audio device (e.g. Sonos/Bluetooth/AirPlay).");
            println!("          Falling back to default format ({} channels, {} Hz).", default_config.channels(), default_config.sample_rate().0);
            println!("          This will cause a slight pitch-shift and conversion warble because the MRT2 engine");
            println!("          runs internally at exactly 48000 Hz.");
            println!("          -> TIP: For pristine sound, use built-in MBP speakers and set them to 48,000 Hz in Audio MIDI Setup!\n");
            default_config
        });
        
    let audio_format_line = format!("{} channels, {} Hz", config_format.channels(), config_format.sample_rate().0);
    println!("Audio Format:  {}", audio_format_line);

    // We pull stereo float samples from the runner and interleave them into the cpal output stream
    let runner_clone = Arc::clone(&runner);
    let stream = device
        .build_output_stream(
            &config_format.into(),
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                let num_frames = data.len() / 2;
                let mut left = vec![0.0f32; num_frames];
                let mut right = vec![0.0f32; num_frames];
                
                // Pull Left/Right stereo samples from the C++ ring buffer (lock-free)
                runner_clone.read_audio_stereo(&mut left, &mut right);
                
                // Interleave them into the cpal hardware buffer
                for (i, frame) in data.chunks_exact_mut(2).enumerate() {
                    frame[0] = left[i];
                    frame[1] = right[i];
                }
            },
            |err| eprintln!("❌ Audio stream error: {}", err),
            None
        )
        .expect("❌ Error: Failed to build CPAL audio output stream");

    // 6. Start the real-time audio playback stream
    stream.play().expect("❌ Error: Failed to start CPAL audio stream");
    println!("✓ Hardware audio output stream started!");

    // 7. Start the C++ real-time inference thread
    println!("Starting real-time playback pipeline...");
    runner.toggle_play(true);

    // dropped_frames is cumulative since engine construction; zero it here so
    // the metrics loop / final recording report only reflect underruns from
    // this session's actual playback, not any pre-existing accumulation from
    // model load or the CPAL device opening before this explicit reset.
    runner.reset_dropped_frames();

    // 8. If --record was requested, capture a fixed-length clip and exit.
    // Note: this pulls from the engine's internal recording buffer, which is
    // populated at MRT2's native 48kHz float PCM directly from the C++ side --
    // independent of whatever sample rate our CPAL live-listening output
    // fell back to (see the 44.1kHz Sonos/Bluetooth warning above). Recorded
    // clips are always pristine native-rate audio regardless of that.
    if record {
        std::fs::create_dir_all(&output_dir).unwrap_or_else(|e| {
            eprintln!("❌ Error: Failed to create output directory {}: {}", output_dir, e);
            std::process::exit(1);
        });

        println!("\n[INFO] Recording {} seconds...", record_seconds);
        runner.start_recording();
        std::thread::sleep(std::time::Duration::from_secs(record_seconds));
        runner.stop_recording();

        let sample_count = runner.get_recorded_sample_count();
        if sample_count == 0 {
            eprintln!("❌ Error: No audio was captured (0 samples recorded).");
            std::process::exit(1);
        }

        let mut left = vec![0.0f32; sample_count];
        let mut right = vec![0.0f32; sample_count];
        runner.get_recorded_audio(0, &mut left, &mut right);

        let filename = format!("recording-{}.wav", chrono::Local::now().format("%Y%m%d-%H%M%S"));
        let out_path = std::path::Path::new(&output_dir).join(&filename);

        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 48000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&out_path, spec).unwrap_or_else(|e| {
            eprintln!("❌ Error: Failed to create WAV file {}: {}", out_path.display(), e);
            std::process::exit(1);
        });
        for i in 0..sample_count {
            let l_i16 = (left[i].clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
            let r_i16 = (right[i].clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
            writer.write_sample(l_i16).ok();
            writer.write_sample(r_i16).ok();
        }
        writer.finalize().unwrap_or_else(|e| {
            eprintln!("❌ Error: Failed to finalize WAV file: {}", e);
            std::process::exit(1);
        });

        println!("✓ Recorded {:.1}s ({} samples) to: {}", 
            sample_count as f64 / 48000.0, sample_count, out_path.display());
        return;
    }

    // Use TUI dashboard when stdout is an interactive terminal.
    // Fall back to the plain scrolling metrics log otherwise (piped output,
    // CI, log files) so we never corrupt non-terminal output streams.
    use std::io::IsTerminal;
    if std::io::stdout().is_terminal() {
        run_tui_dashboard(&runner, &prompt, &model_path, temperature, topk, cfg_text, cfg_drums, audio_format_line);
    } else {
        println!("\n[INFO] Playback running. Press Ctrl+C to stop.");
        let mut count = 0;
        loop {
            std::thread::sleep(std::time::Duration::from_secs(2));
            let metrics_json = runner.read_metrics();
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&metrics_json) {
                let trans_ms = val.get("transformer_ms").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let dropped = val.get("dropped_frames").and_then(|v| v.as_u64()).unwrap_or(0);
                println!("[Metrics] tick: {:03} | transformer: {:.2} ms | dropped frames: {}",
                    count, trans_ms, dropped);
            }
            count += 1;
        }
    }
}

fn run_tui_dashboard(
    runner: &Arc<cxx::UniquePtr<ffi::RealtimeRunnerBridge>>,
    prompt: &str,
    model_path: &Option<String>,
    temperature: f32,
    topk: u32,
    cfg_text: f32,
    cfg_drums: f32,
    audio_format: String,
) {
    // Set up terminal
    enable_raw_mode().expect("❌ Failed to enable raw mode");
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen).expect("❌ Failed to enter alternate screen");
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).expect("❌ Failed to create terminal");

    // History ring for sparkline (30 datapoints ~ 1s at 33 Hz draw rate)
    const HIST: usize = 60;
    let mut trans_history: VecDeque<u64> = VecDeque::with_capacity(HIST);
    let session_start = std::time::Instant::now();

    let mut last_trans_ms = 0.0f64;
    let mut last_dropped: u64 = 0;
    let mut resets: u32 = 0;

    let model_display = model_path
        .as_deref()
        .and_then(|p| std::path::Path::new(p).file_stem())
        .and_then(|s| s.to_str())
        .unwrap_or("none")
        .to_string();

    let draw_tick = std::time::Duration::from_millis(33); // ~30 Hz
    let metrics_tick = std::time::Duration::from_millis(200); // 5 Hz
    let mut last_metrics = std::time::Instant::now();

    let result = (|| -> std::io::Result<()> {
        loop {
            // Poll crossterm events (non-blocking)
            if event::poll(draw_tick)? {
                if let Event::Key(key) = event::read()? {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                        KeyCode::Char('r') => {
                            runner.toggle_play(true);
                            runner.reset_dropped_frames();
                            resets += 1;
                        }
                        _ => {}
                    }
                }
            }

            // Refresh metrics at 5 Hz (not every draw tick -- read_metrics is a JSON alloc)
            if last_metrics.elapsed() >= metrics_tick {
                last_metrics = std::time::Instant::now();
                let metrics_json = runner.read_metrics();
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&metrics_json) {
                    last_trans_ms = val.get("transformer_ms").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    last_dropped = val.get("dropped_frames").and_then(|v| v.as_u64()).unwrap_or(0);
                }
                let sparkval = last_trans_ms.round() as u64;
                trans_history.push_back(sparkval);
                if trans_history.len() > HIST {
                    trans_history.pop_front();
                }
            }

            // Draw frame
            let uptime = session_start.elapsed().as_secs();
            let trans_ms = last_trans_ms;
            let dropped = last_dropped;
            let spark_data: Vec<u64> = trans_history.iter().copied().collect();
            let resets_count = resets;
            let prompt_ref = prompt;
            let model_ref = model_display.as_str();
            let audio_ref = audio_format.as_str();

            // Traffic-light color for transformer latency vs 40ms budget
            let latency_color = if trans_ms < 30.0 {
                Color::Green
            } else if trans_ms < 40.0 {
                Color::Yellow
            } else {
                Color::Red
            };

            // Gauge ratio: how much of the 40ms budget we're using (capped at 100%)
            let budget_ratio = (trans_ms / 40.0).clamp(0.0, 1.0);

            terminal.draw(|f| {
                let area = f.area();
                let rows = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(6),  // session info
                        Constraint::Length(4),  // budget gauge
                        Constraint::Length(8),  // sparkline
                        Constraint::Min(0),     // spacer
                        Constraint::Length(3),  // key help
                    ])
                    .split(area);

                // ── Session info panel ──────────────────────────────────────
                let info_lines = vec![
                    Line::from(vec![
                        Span::styled("  Model    ", Style::default().add_modifier(Modifier::BOLD)),
                        Span::raw(model_ref),
                    ]),
                    Line::from(vec![
                        Span::styled("  Prompt   ", Style::default().add_modifier(Modifier::BOLD)),
                        Span::raw(format!("\"{}\"", prompt_ref)),
                    ]),
                    Line::from(vec![
                        Span::styled("  Params   ", Style::default().add_modifier(Modifier::BOLD)),
                        Span::raw(format!("temp {:.1}  top-k {}  cfg-text {:.1}  cfg-drums {:.1}",
                            temperature, topk, cfg_text, cfg_drums)),
                    ]),
                    Line::from(vec![
                        Span::styled("  Audio    ", Style::default().add_modifier(Modifier::BOLD)),
                        Span::raw(audio_ref),
                    ]),
                    Line::from(vec![
                        Span::styled("  Uptime   ", Style::default().add_modifier(Modifier::BOLD)),
                        Span::raw(format!("{:02}:{:02}:{:02}  resets: {}",
                            uptime / 3600, (uptime % 3600) / 60, uptime % 60, resets_count)),
                    ]),
                ];
                let info = Paragraph::new(info_lines)
                    .block(Block::default()
                        .borders(Borders::ALL)
                        .title(" Magenta RealTime 2 — Rust Player ")
                        .title_style(Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)))
                    .wrap(Wrap { trim: false });
                f.render_widget(info, rows[0]);

                // ── Budget gauge ────────────────────────────────────────────
                let gauge_label = format!(
                    "Transformer  {:.1} ms  /  40.0 ms budget  ({:.0}%)   dropped frames: {}",
                    trans_ms, budget_ratio * 100.0, dropped
                );
                let gauge = Gauge::default()
                    .block(Block::default().borders(Borders::ALL).title(" Frame Budget "))
                    .gauge_style(Style::default().fg(latency_color))
                    .ratio(budget_ratio)
                    .label(gauge_label);
                f.render_widget(gauge, rows[1]);

                // ── Sparkline ───────────────────────────────────────────────
                let spark = Sparkline::default()
                    .block(Block::default()
                        .borders(Borders::ALL)
                        .title(" Transformer ms (last 60 samples) "))
                    .style(Style::default().fg(latency_color))
                    .data(&spark_data);
                f.render_widget(spark, rows[2]);

                // ── Key help bar ────────────────────────────────────────────
                let help = Paragraph::new(Line::from(vec![
                    Span::styled("  q / ESC ", Style::default().fg(Color::Yellow)),
                    Span::raw("quit    "),
                    Span::styled("  r ", Style::default().fg(Color::Yellow)),
                    Span::raw("reset audio context (re-anchors to prompt)    "),
                    Span::styled("  Ctrl-C ", Style::default().fg(Color::Yellow)),
                    Span::raw("quit"),
                ]))
                .block(Block::default().borders(Borders::ALL).title(" Controls "));
                f.render_widget(help, rows[4]);
            })?;
        }
        Ok(())
    })();

    // Always restore the terminal, even on panic/error
    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();

    if let Err(e) = result {
        eprintln!("TUI error: {}", e);
    }

    println!("Playback stopped.");
}
