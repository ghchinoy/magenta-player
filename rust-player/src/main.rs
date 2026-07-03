//! Magenta RealTime 2 — native Rust CLI player.
//!
//! Module layout:
//! - `ffi`    — the cxx bridge to the C++ `magentart::core::RealtimeRunner`
//! - `config` — XDG `config.toml` (AppConfig, load/save) + path helpers
//! - `cli`    — clap arg structs + the `config` subcommand handler
//! - `audio`  — CPAL output stream (+ real-time resampler) and `--record` WAV
//! - `tui`    — the interactive ratatui dashboard
//!
//! `main` just wires argument parsing to the play/config paths, and `run_player`
//! orchestrates the startup sequence (assets -> params -> model -> stream -> play).

mod audio;
mod cli;
mod config;
mod ffi;
mod tui;

use crate::cli::{Cli, Commands, PlayArgs};
use crate::config::{expand_tilde, find_resources_near_model, load_config, AppConfig};
use crate::ffi::ffi::create_runner;
use clap::Parser;
use std::sync::Arc;

fn main() {
    // Initialize standard environment logger
    env_logger::init();

    // Load config from standard XDG path
    let config = load_config();

    // Parse CLI arguments
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Config(cfg_args)) => cli::handle_config(config, cfg_args.action),
        Some(Commands::Play(play_args)) => run_player(config, play_args),
        // Default to play with default arguments if no subcommand is supplied
        None => run_player(config, PlayArgs::default()),
    }
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

    // Snapshot the effective (CLI-over-config-merged) settings as a complete
    // AppConfig. The TUI's explicit "save" key (S) updates the live-adjustable
    // fields on a clone of this and writes it back to config.toml, so a save
    // preserves the non-TUI fields (model, resources, output_dir, cfg_notes,
    // drumless) exactly as they were resolved for this session.
    let effective_config = AppConfig {
        model: model_path.clone(),
        resources: resources_path.clone(),
        prompt: prompt.clone(),
        temperature,
        topk,
        midi_gate,
        cfg_text,
        cfg_notes,
        cfg_drums,
        drumless,
        volume_db,
        output_dir: output_dir.clone(),
    };

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
    let mut runner_unique = create_runner();

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

    // 3. Set the initial generation parameters FIRST so they are ready on startup:
    runner_unique.set_prompt(&prompt);
    runner_unique.set_temperature(temperature);
    runner_unique.set_top_k(topk);
    runner_unique.set_midi_gate(midi_gate);
    runner_unique.set_cfg_text(cfg_text);
    runner_unique.set_cfg_notes(cfg_notes);
    runner_unique.set_cfg_drums(cfg_drums);
    runner_unique.set_drumless(drumless);
    runner_unique.set_volume_db(volume_db);

    // Set ring buffer virtual capacity to 8192 samples (RingBuffer::kCapacity, the
    // physical maximum, ~170ms/4 frames at 48kHz). docs/realtime-audio.md and
    // mrt2-prompt-and-drift.md both call out that 4096 (~85ms) still allows
    // occasional underruns on mrt2_base due to Metal scheduling jitter (50-80ms/frame
    // observed on borderline hardware); 8192 absorbs that variance. This only fixes
    // *jitter*-caused drops -- if dropped_frames still grows unbounded, that's a
    // genuine hardware throughput limit no buffer size can fix (switch to mrt2_small).
    println!("Configuring C++ ring buffer size to 8192 samples (max, ~170ms headroom)...");
    runner_unique.set_buffer_size(8192);

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

    // 6. Open the audio device and build the output stream (with resampler if needed)
    let (stream, audio_format_line, audio_consumer) = audio::build_output_stream(&runner);

    // 7. Start the real-time audio playback stream
    use cpal::traits::StreamTrait;
    stream.play().expect("❌ Error: Failed to start CPAL audio stream");
    println!("✓ Hardware audio output stream started!");

    // 8. Start the C++ real-time inference thread
    println!("Starting real-time playback pipeline...");
    runner.toggle_play(true);

    // dropped_frames is cumulative since engine construction; zero it here so
    // the metrics loop / final recording report only reflect underruns from
    // this session's actual playback, not any pre-existing accumulation from
    // model load or the CPAL device opening before this explicit reset.
    runner.reset_dropped_frames();

    // 9. If --record was requested, capture a fixed-length clip and exit.
    //    Pulls from the engine's internal recording buffer at native 48kHz,
    //    independent of whatever rate the live CPAL output fell back to.
    if record {
        audio::record_to_wav(&runner, &output_dir, record_seconds);
        return;
    }

    // 10. Use TUI dashboard when stdout is an interactive terminal.
    //     Fall back to the plain scrolling metrics log otherwise (piped output,
    //     CI, log files) so we never corrupt non-terminal output streams.
    use std::io::IsTerminal;
    if std::io::stdout().is_terminal() {
        tui::run_tui_dashboard(&runner, effective_config, &model_path, audio_format_line, audio_consumer);
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
