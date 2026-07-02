# Magenta Players & Applications Monorepo

Welcome to the Magenta Players monorepo! This repository is designed to host multiple native and custom frontend players, control surfaces, and companion tools built on top of [Magenta RealTime 2 (MRT2)](https://magenta.withgoogle.com/mrt2), a state-of-the-art open-weights model for real-time local music generation.

## Monorepo Layout

Currently, this repository hosts:

- **[`mrt2-build/`](mrt2-build/)**: The shared C++ build system and artifact directory. It builds the core high-performance Magenta C++ static library (`libmagentart_core.a`) and exports the necessary headers into a single place. This avoids recompiling the heavy C++ engine multiple times across different players.
- **[`swift-player/`](swift-player/)**: A beautiful, fully native SwiftUI macOS application that integrates directly with the Magenta C++ inference engine (`magentart::core::RealtimeRunner`). It supports real-time parameter configuration, performance metrics visualizations, CoreMIDI routing, and on-device save/restore memory states.
- **[`rust-player/`](rust-player/)**: A high-performance, developer-focused command-line interface, prompt daemon, and bidirectional safe FFI wrapper built using **Rust** and **cxx** on top of the C++ Core engine. It is designed for headless background generation, folder automation, and zero-cost concurrent scheduling.

*Future plans include hosting web dashboards, other native mobile players, and automation scripts under this single workspace.*

## Getting Started

The repository utilizes a root-level `Makefile` to orchestrate setup and builds across the shared core and subprojects.

### 1. Build the Shared Core
Before running either of the players, you must compile the C++ core and export headers into `mrt2-build/`:
```bash
# Setup the Python environment and build the static core
make build-mrt2
```

### 2. Download Models and Shared Resources

After `make setup` the `mrt` CLI is available in `mrt2-build/.venv`. Two downloads are required:

**Step A — shared codec and text encoder (one-time, ~500 MB):**
```bash
make mrt-init
```
Downloads `musiccoca/` and `spectrostream/` to `~/Documents/Magenta/magenta-rt-v2/resources/`. Required for audio generation. Only needed once regardless of model size.

**Step B — model weights:**
```bash
make mrt-download MODEL=mrt2_small   # 230M — any Apple Silicon
make mrt-download MODEL=mrt2_base    # 2.4B — M1 Pro/Max or better
```
Downloads the exported `.mlxfn` file to `~/Documents/Magenta/magenta-rt-v2/`. Point the player's **Load Model** picker at the `.mlxfn` file.

**Available models:**

| Model | Parameters | Minimum hardware for real-time |
| :--- | :--- | :--- |
| `mrt2_small` | 230M | Any Apple Silicon (M1 Air and up) |
| `mrt2_base` | 2.4B | M1 Pro / Max and above |

These are the only two sizes available. `mrt2_base` is the largest model.

### 3. Build or Run a Player

Once the shared core is built and a model is downloaded, launch a player:

**Swift Player:**
```bash
make run-swift
```

**Rust Player:**
```bash
make build-rust
```

*(Use `make all` to build everything at once, or see individual project READMEs for details.)*

## Core Tech & Models Used

All players in this repository interface with the [Magenta RealTime 2](https://github.com/magenta/magenta-realtime) system. Models are exported to the `.mlxfn` format for direct loading by the C++ inference engine (`magentart::core::RealtimeRunner`):
- **`mrt2_small`** (230M parameters) — Low-latency, runs in real-time on any Apple Silicon Mac (including Air models).
- **`mrt2_base`** (2.4B parameters) — High-fidelity; requires M1 Pro / Max or better for real-time. This is the largest available model.

## Reference Material

- **Inference Engine & Core Core**: [github.com/magenta/magenta-realtime](https://github.com/magenta/magenta-realtime)
- **Official Website**: [magenta.withgoogle.com/mrt2](https://magenta.withgoogle.com/mrt2)
- **DeepMind MRT2 Documentation**: [magenta.github.io/magenta-realtime/](https://magenta.github.io/magenta-realtime/)

---

## License

This monorepo is licensed under the Apache License 2.0.
