# Magenta Rust Player CLI

A native, high-performance command-line player, prompt daemon, and wrapper for [Magenta RealTime 2 (MRT2)](https://magenta.withgoogle.com/mrt2) built entirely in Rust.

It utilizes Google DeepMind's underlying C++ inference engine (`magentart::core`) through a zero-overhead, lock-free C++ FFI bridge (`cxx`) and streams real-time audio output directly to your sound card using `cpal`.

---

## 🚀 Getting Started & Build Workflows

You can compile and run the Rust player using the root-level orchestrator or through the local directory Makefile.

### Option A: From the Monorepo Root (Recommended)
Before building the player, compile the C++ core library and export its headers once:
```bash
# Compile shared MRT2 static library and populate headers
make build-mrt2

# Compile the Rust player release binary
make build-rust
```
This outputs the fully optimized release binary at `rust-player/target/release/magenta-rust-player`.

---

### Option B: Local Directory Development
If you are developing inside the `rust-player/` directory, a local `Makefile` is provided to wrap standard Cargo development workflows:

```bash
cd rust-player

# 1. Setup local Python/MLX dependencies
make setup

# 2. Build the player in release mode (requires mrt2-build/ completed)
make build

# 3. Compile and run immediately with default settings
make run
```

#### Available Local Makefile Targets:
* `make build` — Compiles the Rust player and FFI bridge in release mode (`cargo build --release`).
* `make run` — Builds and runs the CLI with a default prompt style.
* `make test` — Runs all Rust-side unit and integration tests (`cargo test`).
* `make lint` — Checks code formatting (`cargo fmt`) and runs static analysis lints (`cargo clippy`).
* `make dev` — Launches in hot-reload mode (requires `cargo watch` installed).
* `make clean` — Cleans Cargo targets and build cache.

---

## 🎛️ Reference Model Testing Commands

To test the player, first ensure you have downloaded the MRT2 model sizes using the `mrt` CLI helper:
```bash
# Small model (230M parameters, very fast and low-latency)
../mrt2-build/.venv/bin/mrt models download mrt2_small

# Base model (2.4B parameters, rich fidelity, requires Pro/Max GPU cores)
../mrt2-build/.venv/bin/mrt models download mrt2_base
```

### 1. Test with the Small Model (`mrt2_small`)
This model is highly responsive, lightweight, and runs comfortably on all Apple Silicon chips (including base MacBooks and Airs). 
```bash
./target/release/magenta-rust-player \
  --model ~/Documents/Magenta/magenta-rt-v2/models/mrt2_small/mrt2_small.mlxfn \
  --prompt "ambient lofi chords with acoustic guitar" \
  --temperature 1.3
```
*Note: The RVQ vocoder in the 230M model size has an inherent slightly warbly or "grainy" texture, which is a normal characteristic of the model's small footprint.*

### 2. Test with the Larger Base Model (`mrt2_base`)
For high-fidelity continuations with a full frequency spectrum and clean sound (removing the smaller model's vocoder warble), load the 2.4B base model:
```bash
./target/release/magenta-rust-player \
  --model ~/Documents/Magenta/magenta-rt-v2/models/mrt2_base/mrt2_base.mlxfn \
  --prompt "smooth electric piano chords, rhodes, 90s jazz vibes, clean" \
  --temperature 1.2
```
*Note: Due to the model size, running the base model in real-time requires Pro, Max, or Ultra class chips with sufficient Unified Memory bandwidth.*

---

## 🛠️ CLI Options & Parameters

Customize your real-time generation using these flags:

```text
Options:
  -m, --model <MODEL_PATH>          Path to the model directory or .mlxfn file
  -r, --resources <RESOURCES_PATH>  Path to assets/resources directory [default: ~/Documents/Magenta/magenta-rt-v2/resources]
  -p, --prompt <PROMPT>             Text style conditioning prompt [default: "ambient lofi chords with acoustic guitar"]
  -t, --temperature <TEMPERATURE>   Generation temperature (scales randomness) [default: 1.3]
  -t, --topk <TOPK>                 Top-K token sampling (restricts unlikely choices) [default: 40]
  -m, --midi-gate                   Enable low-latency MIDI gate envelope (only sounds while notes are held)
  -h, --help                        Print help
  -V, --version                     Print version
```

---

## 🧠 Technical Architecture

The safe, zero-overhead FFI bridge is declared in `src/main.rs`:
```rust
#[cxx::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("magenta-rust-player/src/bridge.h");

        type RealtimeRunnerBridge;

        fn create_runner() -> UniquePtr<RealtimeRunnerBridge>;
        fn load_model(self: Pin<&mut RealtimeRunnerBridge>, path: &str) -> bool;
        fn set_prompt(self: Pin<&mut RealtimeRunnerBridge>, prompt: &str);
        fn set_temperature(self: Pin<&mut RealtimeRunnerBridge>, temp: f32);
        fn set_top_k(self: Pin<&mut RealtimeRunnerBridge>, k: u32);
        fn set_midi_gate(self: Pin<&mut RealtimeRunnerBridge>, enabled: bool);
        fn toggle_play(self: Pin<&mut RealtimeRunnerBridge>, playing: bool);
        fn read_audio_stereo(self: &RealtimeRunnerBridge, dest_l: &mut [f32], dest_r: &mut [f32]) -> bool;
        fn read_metrics(self: &RealtimeRunnerBridge) -> String;
    }
}
```

The matching `src/bridge.h` compiles directly alongside the Rust crate. It routes stereo samples from C++ ring buffers into the **CPAL audio thread** using a completely lock-free, zero-mutex pipeline to ensure audio dropouts never occur during MLX GPU tensor calculations.

---

## License

Apache License 2.0
