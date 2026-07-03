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
cfg_text = 3.0     # CFG weight for the style prompt (higher = more adherent, less natural)
cfg_notes = 5.0    # CFG weight for MIDI note conditioning -- see note below, currently a no-op
cfg_drums = 1.0    # CFG weight for drum conditioning (try 0.0 with drumless=false for softer drums)
drumless = false   # Suppress drums entirely, independent of the style prompt
volume_db = 0.0    # Output gain in dB (0.0 = unity gain)
output_dir = "~/Documents/Magenta/magenta-rt-v2/recordings"  # Default folder for --record clips
```

> Config files are forward-compatible: if a future version adds a new field, an older
> `config.toml` missing that key loads fine (the new field is filled in from its default)
> rather than being reset. The file is normalized/rewritten on next load so `config list`
> always reflects the complete, effective settings.

> **⚠️ `resources` is required for `--prompt` to have any effect.** It points at the
> MusicCoCa tokenizer/text-encoder/quantizer assets (`init_assets()`), loaded separately
> from the model weights. If this path is wrong, the player will print a warning on
> startup and silently fall back to the model's default style prompt (a piano loop) no
> matter what `--prompt` you pass. See `docs/realtime-audio.md` for details.
>
> **Auto-fallback:** if the configured `resources` path doesn't exist on disk, the player
> automatically walks up from your `--model`/`config.model` file's directory looking for a
> sibling `resources/` folder before giving up (prints `[INFO] ... auto-derived from model
> location: ...` when this happens). An explicitly-configured path that exists always wins.
>
> **`--cfg-notes` / `cfg_notes` is currently a no-op.** This CLI has no MIDI input wired up
> (unlike the Swift player's CoreMIDI integration), so the "notes held" conditioning signal
> is always empty. The engine's classifier-free guidance compares a positive vs. negative
> conditioning pass to compute the CFG contrast term — with no notes ever held, those two
> passes are identical for the notes signal, so the contrast is exactly zero and `cfg_notes`
> has no audible effect at any value. Kept in the CLI for API completeness / future MIDI
> support, not because changing it currently does anything.

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

# Drumless ambient pad, dialed-down drum CFG, quieter output for this run only
./target/release/magenta-rust-player play --drumless true --cfg-drums 0.0 --volume-db=-6.0

# Capture a 10-second WAV clip to the default folder, then exit
./target/release/magenta-rust-player play --record

# Capture a 30-second clip with a specific prompt, saved to a custom folder
./target/release/magenta-rust-player play --record --record-seconds 30 \
  --prompt "smooth electric piano chords, rhodes, 90s jazz vibes" \
  --output-dir ~/Desktop/mrt2-clips
```

> **Note:** Negative numeric values (e.g. a negative `--volume-db`) must use the `=` form —
> `--volume-db=-6.0` — not `--volume-db -6.0`, otherwise clap mistakes `-6.0` for a flag.
> The same applies to `config set volume_db -6.0`, which works fine since it's a positional arg.

#### Recording (`--record`)

Passing `--record` captures a fixed-length clip and **exits automatically** once saved —
it's a one-shot "give me a WAV to listen to / share" utility, not a background-recording mode.

- **Default folder**: `~/Documents/Magenta/magenta-rt-v2/recordings/` (config key `output_dir`,
  override per-run with `--output-dir <PATH>`; the folder is created automatically if missing).
- **Default filename**: timestamped, e.g. `recording-20260702-181046.wav` (`recording-YYYYMMDD-HHMMSS.wav`),
  so repeated captures never collide or overwrite each other.
- **Default duration**: 10 seconds (`--record-seconds <N>` to change). The actual captured
  duration is reported after saving and may be a little shorter than requested — recording
  starts capturing from whatever moment the engine's internal buffer begins accumulating
  after `start_recording()`, not necessarily instantaneously.
- **Format**: 16-bit PCM WAV, stereo, 48 kHz — pulled directly from the engine's internal
  recording buffer at its native sample rate, **independent of whatever your live CPAL output
  fell back to** (e.g. a 44.1 kHz Sonos/Bluetooth device). Recorded clips are always pristine
  native-rate audio even if what you're *hearing* live has the 44.1 kHz warble described above.

#### `play` Flag Reference
```text
  -m, --model <MODEL_PATH>          Path to the model directory or .mlxfn file
  -r, --resources <RESOURCES_PATH>  Path to the assets/resources directory
  -p, --prompt <PROMPT>             Text style conditioning prompt
  -t, --temperature <TEMPERATURE>   Generation temperature (scales randomness)
  -k, --topk <TOPK>                 Top-K sampling (restricts likely choices)
  -g, --midi-gate <MIDI_GATE>       Enable low-latency MIDI gate envelope [true|false]
      --cfg-text <CFG_TEXT>         CFG weight for the text/style prompt. Factory default: 3.0
      --cfg-notes <CFG_NOTES>       CFG weight for MIDI note conditioning. Factory default: 5.0
      --cfg-drums <CFG_DRUMS>       CFG weight for drum conditioning. Factory default: 1.0
      --drumless <DRUMLESS>         Suppress drums entirely, independent of the style prompt [true|false]
      --volume-db <VOLUME_DB>       Output gain in dB (0.0 = unity gain)
      --record                      Record a WAV clip of this session and exit once done
      --record-seconds <SECONDS>    Duration to record when --record is set [default: 10]
      --output-dir <OUTPUT_DIR>     Directory to save recorded WAV clips into (overrides config output_dir)
  -h, --help                        Print help
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

# Tune CFG weights, drums, and volume as persistent defaults
./target/release/magenta-rust-player config set cfg_text 4.0
./target/release/magenta-rust-player config set drumless true
./target/release/magenta-rust-player config set volume_db -6.0

# Clear the default model path from config
./target/release/magenta-rust-player config set model none
```

**Valid `config set` keys**: `model`, `resources`, `prompt`, `temperature`, `topk`, `midi_gate`, `cfg_text`, `cfg_notes`, `cfg_drums`, `drumless`, `volume_db`, `output_dir`.

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

### Live TUI Dashboard

When running interactively (attached to a real terminal), `play` launches a live
`ratatui` + `crossterm` full-screen dashboard instead of scrolling log lines:

| Panel | Content |
|---|---|
| **Session info** | Model, prompt, params (temperature/top-k/CFGs), audio format, uptime, reset count |
| **Frame budget gauge** | Transformer latency vs 40ms real-time budget, traffic-light coloured (green < 30ms / yellow < 40ms / red over-budget) |
| **Sparkline** | Rolling 60-sample history of transformer latency (ms) |
| **Controls bar** | Keyboard shortcuts |

**Keyboard controls (interactive only):**
* **Inference Parameters (Live Steering)**:
  * `[` / `]` — Decrease / Increase **Prompt Strength** (`cfg_text`) by `0.5` (range: `1.0` - `10.0`)
  * `-` / `+` — Decrease / Increase **Temperature** by `0.1` (range: `0.1` - `2.5`, `=` key works as `+` too)
  * `,` / `.` — Decrease / Increase **Top-K** sampling by `5` (range: `5` - `200`, `<` / `>` work too)
  * `d` / `f` — Decrease / Increase **Drums CFG** (`cfg_drums`) by `0.5` (range: `0.0` - `10.0`)
* **Audio & MIDI**:
  * `v` / `b` — Decrease / Increase **Volume** (`volume_db`) by `2.0` dB (range: `-60.0` - `12.0` dB)
  * `g`         — Toggle **MIDI Gate** on/off
* **Session Lifecycle**:
  * `r`         — Trigger **audio context reset** mid-playback (re-anchors generation immediately to the current prompt with no audio gap)
  * `q` / `ESC` — Quit player cleanly and restore terminal
  * `Ctrl-C`   — Quit player cleanly and restore terminal

Falls back to the plain scrolling metrics log when stdout is not a TTY (piped output, CI, log files, `--record` mode) — the TUI never corrupts non-terminal output streams.

### Loading Indicators

The three blocking/async startup phases (MusicCoCa asset init, model load, prompt encoding)
show an animated `indicatif` spinner in an interactive terminal. Each phase always prints a
plain status line first/after (`Loading model from: ...` / `✓ Model loaded successfully!`)
regardless of TTY state — indicatif suppresses spinner drawing entirely when stderr isn't a
terminal (piped output, log files, CI), so the spinner is purely cosmetic; the actual status
information is never spinner-only.

---

## License

Apache License 2.0
