//! Safe C++ FFI bridge to the Magenta RealTime 2 `magentart::core::RealtimeRunner`.
//!
//! The `#[cxx::bridge]` block below is compiled by `cxx-build` (see `build.rs`,
//! which points `cxx_build::bridge()` at this file) together with the C++ glue
//! in `src/bridge.h`.
//!
//! Threading note: nearly all setters and the audio/metrics readers are declared
//! `self: &RealtimeRunnerBridge` (shared ref) because the underlying C++ methods
//! operate on atomics / lock-free ring buffers and are safe to call concurrently.
//! Only the two blocking lifecycle calls (`init_assets`, `load_model`) take
//! `Pin<&mut ...>` and are used once during startup. See `bridge.h` for the
//! `const`/`const_cast` details.

// Bidirectional safe FFI bridge using cxx.
// The inner module must be named `ffi` for the #[cxx::bridge] macro; allow the
// module-inception lint since ffi.rs -> mod ffi is intentional here.
#[cxx::bridge]
#[allow(clippy::module_inception)]
pub mod ffi {
    unsafe extern "C++" {
        include!("magenta-rust-player/src/bridge.h");

        // We can expose the C++ RealtimeRunner class directly to Rust
        type RealtimeRunnerBridge;

        fn create_runner() -> UniquePtr<RealtimeRunnerBridge>;
        fn init_assets(self: Pin<&mut RealtimeRunnerBridge>, resource_dir: &str) -> bool;
        fn load_model(self: Pin<&mut RealtimeRunnerBridge>, path: &str) -> bool;
        fn set_prompt(self: &RealtimeRunnerBridge, prompt: &str);
        fn set_temperature(self: &RealtimeRunnerBridge, temp: f32);
        fn set_top_k(self: &RealtimeRunnerBridge, k: u32);
        fn set_midi_gate(self: &RealtimeRunnerBridge, enabled: bool);
        fn set_buffer_size(self: &RealtimeRunnerBridge, cap: usize);
        fn set_cfg_text(self: &RealtimeRunnerBridge, v: f32);
        fn set_cfg_notes(self: &RealtimeRunnerBridge, v: f32);
        fn set_cfg_drums(self: &RealtimeRunnerBridge, v: f32);
        fn set_drumless(self: &RealtimeRunnerBridge, on: bool);
        fn set_volume_db(self: &RealtimeRunnerBridge, v: f32);
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
// Safe because RealtimeRunner's lifecycle methods are internally mutex-serialized,
// parameter setters are atomic, and read_audio_stereo is fully lock-free.
unsafe impl Send for ffi::RealtimeRunnerBridge {}
unsafe impl Sync for ffi::RealtimeRunnerBridge {}
