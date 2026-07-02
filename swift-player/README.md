# Magenta Swift Player

A native Swift-based UI player for [Magenta RealTime 2](https://magenta.withgoogle.com/mrt2), built on top of the C++ inference engine. This project demonstrates how to use Swift and SwiftUI to create a beautiful, reactive, native macOS interface for real-time local music generation.

## Key Features

- **Native SwiftUI Interface**: A fully native, modern macOS control surface mimicking the visual style and features of the original Magenta UI.
- **Direct C++ Engine Connection**: Blends natively with the `magentart::core::RealtimeRunner` engine.
- **Real-Time Controls**: Interactive knobs and sliders for Temperature, Top-K, Prompt Strength, Note Strength, and Auto-Strum.
- **Performance Visualization**: Level meters for audio output, active MIDI note visualizations, and real-time inference thread latency tracking.
- **Memory Banks**: Snapshots to save, load, and restore the model's audio state (up to the last 20 seconds of context).
- **CoreMIDI Integration**: Supports both physical hardware MIDI inputs and virtual MIDI endpoints so other applications (e.g., DAWs) can route notes directly into the generator.

---

## Quick Start

> **Preferred entry point**: run these commands from the **repo root** (`magenta-player/`), not from this directory. The root `Makefile` orchestrates the shared C++ engine build in `mrt2-build/` and then delegates to this player.
>
> ```bash
> cd magenta-player   # repo root
> make setup
> make build-mrt2
> make run-swift
> ```
>
> Everything below also works when run from inside `swift-player/` — `make setup` and `make build-mrt2` will delegate up to `../mrt2-build` automatically.

### 1. Prerequisites

- **Apple Silicon Mac**: Generating audio faster than playback requires Apple Silicon (M1 or later).
- **macOS 12.0** or newer.
- **Xcode** 14.0 or newer (with Command Line Tools installed).
- **uv**: An ultra-fast Python package installer. Install it with:
  ```bash
  curl -LsSf https://astral.sh/uv/install.sh | sh
  ```

### 2. Metal Toolchain Requirement

Since the inference engine compiles custom Metal kernels for Apple Silicon GPU acceleration, ensure you have the required Metal Toolchain installed:
```bash
xcodebuild -downloadComponent MetalToolchain
```

### 3. Build Instructions

```bash
# 1. Clone magenta-realtime into mrt2-build/ and install Python/MLX deps
make setup

# 2. Build the shared C++ engine (libmagentart-core.a) into mrt2-build/
make build-mrt2

# 3. Build the native Swift player app (release binary)
make build-swift
```

*Note: `make all` runs all three steps sequentially (but not the model download — that is a separate step).*

### 4. Download Models and Shared Resources

Before launching the app you need two things on disk. Both commands are run
from the **repo root** — no venv activation needed, `uv` handles everything.

**Step A — shared codec and text encoder (one-time, ~500 MB):**
```bash
make mrt-init
```
Downloads `musiccoca/` and `spectrostream/` into
`~/Documents/Magenta/magenta-rt-v2/resources/`. Shared across all model sizes;
only needs to be done once.

**Step B — model weights:**
```bash
make mrt-download MODEL=mrt2_small    # 230M — real-time on any Apple Silicon
make mrt-download MODEL=mrt2_base     # 2.4B — requires M1 Pro/Max or better
```
Downloads the exported `.mlxfn` model to `~/Documents/Magenta/magenta-rt-v2/`.
Point the **Load Model…** file picker (`Cmd+O`) at the `.mlxfn` file.

**Available models:**

| Model | Parameters | Real-time on |
| :--- | :--- | :--- |
| `mrt2_small` | 230M | Any Apple Silicon (M1 Air and up) |
| `mrt2_base` | 2.4B | M1 Pro / Max and above |

> **Tip:** `mrt2_small` is the right starting point. `mrt2_base` produces
> higher-fidelity output but requires a Pro or Max chip to run at 25 Hz.

A future release will add an in-app **Download Model** sheet that removes
the need for this terminal step (tracked as `magenta-player-rdt`).

### 5. Run the App

After a successful build, launch the player with:

```bash
make run-swift
```

This uses `swift run` under the hood — it performs an incremental rebuild if any Swift source files have changed, then launches the app. The window appears immediately on your current desktop space.

If you prefer to run the release binary directly (e.g. in a CI context after `make build-swift`):

```
.build/release/magenta-player
# also copied to:
.build/MRT2
```

> **Tip:** Use `Cmd+O` (or **File > Load Model…** in the menu bar) to open the model picker once the app is running. `Space` toggles Play/Stop.

---

## Makefile Target Reference

| Target | Description |
| :--- | :--- |
| `make setup` | Delegates to `../mrt2-build`: clones `magenta-realtime` into `mrt2-build/` and installs Python/MLX dependencies (including the `mrt` CLI). |
| `make build-mrt2` | Delegates to `../mrt2-build`: compiles `libmagentart_core.a`, merges all transitive deps into `libmagentart_all.a`, and copies public headers to `mrt2-build/include/`. |
| `make build-swift` | Compiles Swift Package Manager targets into a release binary at `.build/release/magenta-player`. Depends on `build-mrt2`. |
| `make run-swift` | Incremental Swift build + immediate launch. Does not re-run the C++ build. Equivalent to `swift run magenta-player`. |
| `make all` | `build-mrt2` → `build-swift`. |
| `make clean` | Removes `.build/` (Swift artifacts only). The shared engine is cleaned via `make clean-mrt2` from the repo root. |
| `make clean-legacy` | One-time cleanup: removes the old `swift-player/magenta-realtime/` clone left over from before the `mrt2-build` migration. Safe to run multiple times. |

---

## Architecture

The `swift-player` sits on top of two layers:

```
magenta-player/
├── mrt2-build/               ← shared C++ engine (libmagentart-core.a + headers)
└── swift-player/
    ├── Package.swift         ← Swift package (will link mrt2-build when bridge is wired)
    └── src/
        ├── MagentaPlayerApp.swift   ← @main entry point, menu bar commands
        ├── Models.swift             ← PlayerState, EngineMetrics, ParameterValues
        ├── Managers.swift           ← AudioEngineManager, PlayerManager, MIDIManager
        └── Views/
            ├── PlayerView.swift     ← main SwiftUI layout
            └── ViewExtensions.swift ← errorAlert modifier
```

The Swift source follows a clean MVVM layering:

- **View Layer (`src/Views/`)**: Declarative SwiftUI controls — Play/Stop, Load Model, level meters, performance metrics, error alerts.
- **Model Layer (`src/Models.swift`)**: Struct-based value types for `PlayerState`, `EngineMetrics`, `ParameterValues`, and `MIDIEvent`.
- **Manager Layer (`src/Managers.swift`)**:
  - `AudioEngineManager`: lock-free `AVAudioSourceNode` at 48 kHz stereo.
  - `PlayerManager` (`@MainActor`): reactive state owner; will coordinate with the C++ bridge once wired.
  - `MIDIManager`: CoreMIDI virtual + physical input port.
- **Entry Point (`src/MagentaPlayerApp.swift`)**: `@main` scene with `.commands` block providing `Cmd+O` (Load Model) and `Space` (Play/Stop) menu bar shortcuts.

---

## Hardware Support Matrix

Real-time streaming requires Apple Silicon due to the intensive local GPU inference.

| Device | `mrt2_small` (230M parameters) | `mrt2_base` (2.4B parameters) |
| :--- | :---: | :---: |
| **M5 Max / M4 Pro / M3 Pro / M2 Max** | ✅ Real-Time | ✅ Real-Time |
| **M1 Pro / M2 Pro / M1 Max** | ✅ Real-Time | ❌ Offline Only |
| **M1 Air / M2 Air / M3 Air / M4 Air** | ✅ Real-Time | ❌ Offline Only |

---

## Important Links

- **Magenta RealTime Repository**: [github.com/magenta/magenta-realtime](https://github.com/magenta/magenta-realtime)
- **Magenta RealTime Landing Page**: [magenta.withgoogle.com/mrt2](https://magenta.withgoogle.com/mrt2)
- **Full Documentation & Book**: [magenta.github.io/magenta-realtime/](https://magenta.github.io/magenta-realtime/)

---

## Development & Lessons Learned

During the transition from an Objective-C++/React hybrid stack to a purely native Swift/SwiftUI application, we documented critical engineering constraints regarding:
- Native lock-free real-time audio thread scheduling.
- Bidirectional parameter state synchronization.
- C++ / Swift memory bounds interop.

For a deep-dive, see:
- **[DEVELOPMENT.md](DEVELOPMENT.md)**: File-by-file explanations and setup logic.
- **[LESSONS_LEARNED.md](LESSONS_LEARNED.md)**: 10 structural lessons learned for building real-time Apple Silicon audio systems.

## License

Apache License 2.0
