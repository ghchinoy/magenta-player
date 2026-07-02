# Magenta Rust Player CLI

A native, high-performance command-line player, prompt daemon, and wrapper for [Magenta RealTime 2 (MRT2)](https://magenta.withgoogle.com/mrt2) built entirely in Rust.

It utilizes Google DeepMind's underlying C++ inference engine (`magentart::core`) through a zero-overhead, lock-free C++ FFI bridge (`cxx`) and streams real-time audio output directly to your sound card using `cpal`.

---

## 📂 XDG Configuration Support (Viper-style)

The player supports persistent configuration settings stored in your platform's standard XDG configuration folder:
* **macOS Path**: `~/Library/Application Support/magenta-rust-player/config.toml`
* **Linux Path**: `~/.config/magenta-rust-player/config.toml`

When launching the player with no arguments (simply running `./target/release/magenta-rust-player`), it will **automatically** load your saved default configuration—including the model path—meaning you only need to configure it once!

### Standard `config.toml` Format:
```toml
model = "/Users/username/Documents/Magenta/magenta-rt-v2/models/mrt2_small/mrt2_small.mlxfn"
resources = "~/Documents/Magenta/magenta-rt-v2/resources"
prompt = "ambient lofi chords with acoustic guitar"
temperature = 1.3
topk = 40
midi_gate = false
```

> **⚠️ `resources` is required for `--prompt` to have any effect.** It points at the
> MusicCoCa tokenizer/text-encoder/quantizer assets (`init_assets()`), loaded separately
> from the model weights. If this path is wrong, the player will print a warning on
> startup and silently fall back to the model's default style prompt (a piano loop) no
> matter what `--prompt` you pass. See `docs/realtime-audio.md` for details.

---

## 🛠️ CLI Subcommands & Usage

The Rust player supports declarative, Cobra-like subcommands using `clap`. Any command-line arguments passed to the `play` subcommand will dynamically override your saved `config.toml` defaults for that run.

### 1. The `play` Subcommand
Launches real-time audio streaming.
```bash
# Starts playback using your config.toml defaults
./target/release/magenta-rust-player

# Same as above, but explicit
./target/release/magenta-rust-player play

# Overrides the default prompt and temperature for this run only
./target/release/magenta-rust-player play --prompt "lofi jazz piano" --temperature 1.1
```

### 2. The `config` Subcommand
Inspects and modifies your saved defaults.

```bash
# View the path and contents of your active config.toml
./target/release/magenta-rust-player config list

# Print only the absolute file path to config.toml
./target/release/magenta-rust-player config path

# Modify a specific parameter in your config file permanently
./target/release/magenta-rust-player config set prompt "ambient chill step synthesizer"
./target/release/magenta-rust-player config set temperature 1.1
./target/release/magenta-rust-player config set model "/path/to/mrt2_base/mrt2_base.mlxfn"

# Clear the default model path from config
./target/release/magenta-rust-player config set model none
```

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
./target/release/magenta-rust-player play \
  --model ~/Documents/Magenta/magenta-rt-v2/models/mrt2_small/mrt2_small.mlxfn \
  --prompt "ambient lofi chords with acoustic guitar" \
  --temperature 1.3
```
*Note 1: The RVQ vocoder in the 230M model size has an inherent slightly warbly or "grainy" texture, which is a normal characteristic of the model's small footprint.*

*Note 2: If you are using external speakers (like Sonos Roam or Bluetooth) that lock to 44.1 kHz instead of 48 kHz, the player will show a warning. It will fall back to 44.1 kHz, which causes a minor pitch-shift and conversion warble. For pristine sound, use built-in MBP speakers set to 48,000 Hz in macOS Audio MIDI Setup.*

### 2. Test with the Larger Base Model (`mrt2_base`)
For high-fidelity continuations with a full frequency spectrum and clean sound (removing the smaller model's vocoder warble), load the 2.4B base model:
```bash
./target/release/magenta-rust-player play \
  --model ~/Documents/Magenta/magenta-rt-v2/models/mrt2_base/mrt2_base.mlxfn \
  --prompt "smooth electric piano chords, rhodes, 90s jazz vibes, clean" \
  --temperature 1.2
```
*Note: Due to the model size, running the base model in real-time requires Pro, Max, or Ultra class chips with sufficient Unified Memory bandwidth.*

---

## 🚀 Getting Started & Build Workflows

You can compile the Rust player using the root-level orchestrator or through the local directory Makefile.

### Option A: From the Monorepo Root (Recommended)
Before building the player, compile the C++ core library and export its headers once:
```bash
# Compile shared MRT2 static library and populate headers
make build-mrt2

# Compile the Rust player release binary
make build-rust
```
This outputs the fully optimized release binary at `rust-player/target/release/magenta-rust-player`.

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

---

## 🧠 Technical Architecture

The FFI boundary and CPAL real-time stream integration is declared natively inside `src/main.rs`.

The C++ bridge in `src/bridge.h` compiles directly alongside the Rust crate. It routes stereo samples from C++ ring buffers into the **CPAL audio thread** using a completely lock-free, zero-mutex pipeline, ensuring that intensive MLX GPU tensor calculations never block the real-time audio output.

---

## License

Apache License 2.0
