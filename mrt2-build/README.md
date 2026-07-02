# mrt2-build — Shared MRT2 C++ Engine

This directory builds the [Magenta RealTime 2](https://github.com/magenta/magenta-realtime) C++ inference engine once and exposes its outputs to every player in this monorepo. No player needs to clone or compile the engine on its own.

## What this produces

```
mrt2-build/
├── libmagentart-core.a       ← static library (link target for all players)
└── include/
    └── magentart/
        ├── realtime_runner.h ← primary public API
        ├── mlx_engine.h
        └── detail/           ← implementation details (included transitively)
```

Both the library and headers are **git-ignored** — they are build artifacts, not source. Run `make build-mrt2` to generate them.

## Quick start

From the **repo root** (preferred — the root `Makefile` orchestrates everything):

```bash
make setup       # clone magenta-realtime + install Python/MLX deps
make build-mrt2  # compile libmagentart-core.a and copy headers here
```

Or directly from this directory:

```bash
make setup
make build-mrt2
```

## Targets

| Target | Description |
| :--- | :--- |
| `make setup` | Creates a Python 3.12 venv at `mrt2-build/.venv`, clones `magenta-realtime` into `mrt2-build/magenta-realtime/`, and installs the `magenta_rt[mlx]` Python package (provides the `mrt` CLI for model downloads). |
| `make build-mrt2` | Runs `cmake` + `cmake --build` targeting `magentart-core`, then copies `libmagentart-core.a` and all public headers into this directory. Uses `cmake -S/-B` syntax — safe to call from any working directory. |
| `make clean` | Removes the cloned source, compiled library, copied headers, and `.venv`. |
| `make help` | Prints target list and resolved output paths. |

## Overriding the source location

If you already have a `magenta-realtime` checkout elsewhere, skip the clone and point the build at it:

```bash
make build-mrt2 MAGENTA_REALTIME_DIR=/path/to/magenta-realtime
```

This is useful in CI where the repo is checked out independently, or when working on the engine itself alongside a player.

## How players consume this

Each player Makefile accepts a `MRT2_BUILD_DIR` variable (defaults to `../mrt2-build`). The root Makefile exports it as an environment variable so sub-makes inherit it automatically.

**Swift player** (`swift-player/Makefile`):
```bash
make setup      # → make -C ../mrt2-build setup
make build-mrt2 # → make -C ../mrt2-build build-mrt2
```
The `Package.swift` will link `libmagentart-core.a` once the Swift–C++ bridge is wired up.

**Rust player** (`rust-player/`):
A `build.rs` reads `MRT2_BUILD_DIR` and passes `libmagentart-core.a` and `include/` to `cxx-build`. See `rust-player/README.md` for details.

## Directory contents (annotated)

```
mrt2-build/
├── Makefile              ← build orchestration (this is the source of truth)
├── README.md             ← this file
├── .gitignore            ← ignores magenta-realtime/, libmagentart-core.a, .venv/
├── include/
│   └── magentart/        ← populated by `make build-mrt2` (git-ignored)
├── magenta-realtime/     ← cloned by `make setup` (git-ignored)
└── .venv/                ← Python env for `mrt` CLI (git-ignored)
```

## Prerequisites

- **cmake** 3.5 or newer
- **git**
- **uv** — `curl -LsSf https://astral.sh/uv/install.sh | sh`
- **Xcode Command Line Tools** (macOS) — `xcode-select --install`
- **Metal Toolchain** (for GPU kernel compilation) — `xcodebuild -downloadComponent MetalToolchain`
