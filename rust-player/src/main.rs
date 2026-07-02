// This file demonstrates how to build a native Rust command-line tool
// that bridges to the Magenta RealTime 2 C++ inference engine using 'cxx'.
//
// Since 'magentart::core' is written in high-performance C++, Rust can safely
// coordinate the file-watch pipelines, MIDI listeners, and config systems
// without any garbage collection or FFI translation overhead.

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "magenta-rust", author, version, about = "Rust CLI Player for Magenta RealTime 2", long_about = None)]
struct Args {
    /// Path to the model directory or .mlxfn file
    #[arg(short, long, value_name = "MODEL_PATH")]
    model: Option<PathBuf>,

    /// Path to the assets/resources directory (MusicCoCa + SpectroStream)
    #[arg(short, long, value_name = "RESOURCES_PATH", default_value = "~/Documents/Magenta/magenta-rt-v2/resources")]
    resources: String,

    /// Text style conditioning prompt
    #[arg(short, long, default_value = "ambient lofi chords with acoustic guitar")]
    prompt: String,

    /// Generation temperature (scales unpredictability)
    #[arg(short, long, default_value_t = 1.3)]
    temperature: f32,

    /// Top-K sampling (restricts likely choices)
    #[arg(short, long, default_value_t = 40)]
    topk: u32,

    /// Enable low-latency MIDI gate envelope
    #[arg(short, long)]
    midi_gate: bool,
}

// Bidirectional safe FFI bridge using cxx
#[cxx::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("magenta-rust-player/src/bridge.h");

        // We can expose the C++ RealtimeRunner class directly to Rust
        type RealtimeRunnerBridge;

        fn create_runner() -> UniquePtr<RealtimeRunnerBridge>;
        fn load_model(self: Pin<&mut RealtimeRunnerBridge>, path: &str) -> bool;
        fn set_prompt(self: Pin<&mut RealtimeRunnerBridge>, prompt: &str);
        fn set_temperature(self: Pin<&mut RealtimeRunnerBridge>, temp: f32);
        fn set_top_k(self: Pin<&mut RealtimeRunnerBridge>, k: u32);
        fn set_midi_gate(self: Pin<&mut RealtimeRunnerBridge>, enabled: bool);
        fn set_buffer_size(self: Pin<&mut RealtimeRunnerBridge>, cap: usize);
        fn toggle_play(self: &RealtimeRunnerBridge, playing: bool);
        fn read_audio_stereo(
            self: &RealtimeRunnerBridge,
            dest_l: &mut [f32],
            dest_r: &mut [f32],
        ) -> bool;
        fn read_metrics(self: &RealtimeRunnerBridge) -> String;
    }
}

// Implement Send and Sync so we can share the runner with the cpal audio callback thread safely.
// This is safe because RealtimeRunner's lifecycle methods are internally mutex-serialized,
// parameter setters are atomic, and read_audio_stereo is fully lock-free.
unsafe impl Send for ffi::RealtimeRunnerBridge {}
unsafe impl Sync for ffi::RealtimeRunnerBridge {}

use std::sync::Arc;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

fn main() {
    // Initialize standard environment logger
    env_logger::init();
    
    let args = Args::parse();
    
    println!("=== Magenta RealTime 2 Rust Player CLI ===");
    println!("Prompt:      \"{}\"", args.prompt);
    println!("Temperature: {}", args.temperature);
    println!("Top-K:       {}", args.topk);
    println!("MIDI Gate:   {}", if args.midi_gate { "Enabled" } else { "Disabled" });
    println!("=========================================");

    // 1. Initialize the C++ RealtimeRunner via the cxx bridge:
    println!("\nInitializing C++ RealtimeRunner...");
    let mut runner_unique = ffi::create_runner();

    // 2. Load the model and resources if provided:
    if let Some(ref path) = args.model {
        let path_str = path.to_string_lossy();
        println!("Loading model from: {}", path_str);
        if runner_unique.pin_mut().load_model(&path_str) {
            println!("✓ Model loaded successfully!");
        } else {
            eprintln!("❌ Error: Failed to load model from {}", path_str);
            std::process::exit(1);
        }
    } else {
        println!("[WARNING] No model path specified. Use -m or --model to load an MRT2 model.");
    }

    // 3. Set the initial generation parameters:
    runner_unique.pin_mut().set_prompt(&args.prompt);
    runner_unique.pin_mut().set_temperature(args.temperature);
    runner_unique.pin_mut().set_top_k(args.topk);
    runner_unique.pin_mut().set_midi_gate(args.midi_gate);
    
    // Set ring buffer virtual capacity to 4096 samples (2 frames at 48kHz) 
    // as per best practices in docs/realtime-audio.md to absorb GPU scheduling jitter
    println!("Configuring C++ ring buffer size to 4096 samples...");
    runner_unique.pin_mut().set_buffer_size(4096);

    // 4. Wrap runner in Arc to share with the cpal audio thread
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
        
    let config = supported_configs_range
        .filter(|c| c.channels() == 2)
        .find(|c| c.min_sample_rate().0 <= 48000 && c.max_sample_rate().0 >= 48000)
        .map(|c| c.with_sample_rate(cpal::SampleRate(48000)))
        .unwrap_or_else(|| {
            println!("[WARNING] 48kHz stereo format not directly supported. Falling back to default format.");
            device.default_output_config().expect("❌ Error: Failed to get default audio output configuration")
        });
        
    println!("Audio Format:  {} channels, {} Hz", config.channels(), config.sample_rate().0);

    // We pull stereo float samples from the runner and interleave them into the cpal output stream
    let runner_clone = Arc::clone(&runner);
    let stream = device
        .build_output_stream(
            &config.into(),
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
