use clap::{Args as ClapArgs, Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

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
        fn toggle_play(self: &RealtimeRunnerBridge, playing: bool);
        fn get_quantizer_status(self: &RealtimeRunnerBridge) -> i32;
        fn read_audio_stereo(
            self: &RealtimeRunnerBridge,
            dest_l: &mut [f32],
            dest_r: &mut [f32],
        ) -> bool;
        fn read_metrics(self: &RealtimeRunnerBridge) -> String;
    }
}

// Implement Send and Sync so we can share the runner with the cpal audio callback thread safely.
unsafe impl Send for ffi::RealtimeRunnerBridge {}
unsafe impl Sync for ffi::RealtimeRunnerBridge {}

/// Configuration Structure for XDG Config Support
#[derive(Serialize, Deserialize, Debug, Clone)]
struct AppConfig {
    model: Option<String>,
    resources: String,
    prompt: String,
    temperature: f32,
    topk: u32,
    midi_gate: bool,
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
            if let Ok(config) = toml::from_str(&content) {
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

#[derive(ClapArgs, Debug)]
struct PlayArgs {
    /// Path to the model directory or .mlxfn file
    #[arg(short, long, value_name = "MODEL_PATH")]
    model: Option<PathBuf>,

    /// Path to the assets/resources directory
    #[arg(short, long, value_name = "RESOURCES_PATH")]
    resources: Option<String>,

    /// Text style conditioning prompt
    #[arg(short, long)]
    prompt: Option<String>,

    /// Generation temperature (scales randomness)
    #[arg(short, long)]
    temperature: Option<f32>,

    /// Top-K sampling (restricts likely choices)
    #[arg(short, long)]
    topk: Option<u32>,

    /// Enable low-latency MIDI gate envelope
    #[arg(short, long)]
    midi_gate: Option<bool>,
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
        /// Configuration key to modify (model, resources, prompt, temperature, topk, midi_gate)
        key: String,
        
        /// New value for the configuration key
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
                        _ => {
                            eprintln!("❌ Error: Unknown configuration key '{}'", key);
                            eprintln!("Valid keys: model, resources, prompt, temperature, topk, midi_gate");
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
            run_player(config, PlayArgs {
                model: None,
                resources: None,
                prompt: None,
                temperature: None,
                topk: None,
                midi_gate: None,
            });
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

fn run_player(config: AppConfig, args: PlayArgs) {
    // Merge logic: CLI arguments override XDG config file defaults
    let model_path = args.model.map(|p| p.to_string_lossy().to_string()).or(config.model);
    let resources_path = expand_tilde(&args.resources.unwrap_or(config.resources));
    let prompt = args.prompt.unwrap_or(config.prompt);
    let temperature = args.temperature.unwrap_or(config.temperature);
    let topk = args.topk.unwrap_or(config.topk);
    let midi_gate = args.midi_gate.unwrap_or(config.midi_gate);

    println!("=== Magenta RealTime 2 Rust Player CLI ===");
    println!("Prompt:      \"{}\"", prompt);
    println!("Temperature: {}", temperature);
    println!("Top-K:       {}", topk);
    println!("MIDI Gate:   {}", if midi_gate { "Enabled" } else { "Disabled" });
    println!("Resources:   {}", resources_path);
    if let Some(ref path) = model_path {
        println!("Model Path:  {}", path);
    } else {
        println!("Model Path:  None");
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
    if runner_unique.pin_mut().init_assets(&resources_path) {
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
    
    // Set ring buffer virtual capacity to 4096 samples (2 frames at 48kHz) 
    // as per best practices in docs/realtime-audio.md to absorb GPU scheduling jitter
    println!("Configuring C++ ring buffer size to 4096 samples...");
    runner_unique.pin_mut().set_buffer_size(4096);

    // 4. Load the model if provided:
    if let Some(ref path_str) = model_path {
        let expanded_path = expand_tilde(path_str);
        println!("Loading model from: {}", expanded_path);
        if runner_unique.pin_mut().load_model(&expanded_path) {
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
    use std::io::Write;
    print!("Encoding style prompt");
    std::io::stdout().flush().ok();
    let encode_timeout = std::time::Duration::from_secs(5);
    let encode_start = std::time::Instant::now();
    loop {
        let status = runner_unique.get_quantizer_status();
        if status == 2 {
            println!(" done!");
            break;
        }
        if status == 3 {
            println!();
            eprintln!("[WARNING] Prompt encoding failed (status=3). Check that --resources points at a valid magenta-rt-v2/resources directory.");
            break;
        }
        if encode_start.elapsed() > encode_timeout {
            println!();
            eprintln!("[WARNING] Timed out waiting for prompt encoding after {:?}; starting anyway.", encode_timeout);
            break;
        }
        print!(".");
        std::io::stdout().flush().ok();
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
        
    println!("Audio Format:  {} channels, {} Hz", config_format.channels(), config_format.sample_rate().0);

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

    println!("\n[INFO] Playback running. Press Ctrl+C to stop.");
    
    // Poll and print live metrics from the engine every 2 seconds
    let mut count = 0;
    loop {
        std::thread::sleep(std::time::Duration::from_secs(2));
        let metrics_json = runner.read_metrics();
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&metrics_json) {
            let trans_ms = val.get("transformer_ms").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let dropped = val.get("dropped_frames").and_then(|v| v.as_u64()).unwrap_or(0);
            println!("[Metrics] tick: {:03} | transformer: {:.2} ms | dropped frames: {}", 
                count, 
                trans_ms,
                dropped
            );
        }
        count += 1;
    }
}
