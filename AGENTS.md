# AI Agent Workspace Instructions & Roles

This workspace is a monorepo containing multiple high-performance players and wrappers for Google DeepMind's **Magenta RealTime 2 (MRT2)** inference engine.

---

## 🎯 Monorepo Goal

The goal of this monorepo is to provide multiple language-specific implementations of real-time wrappers/players around the core MRT2 C++ inference engine:
1. **Shared Core (`mrt2-build/`)**: A single, unified compilation artifact directory containing the compiled C++ static library (`libmagentart-core.a`) and matching headers. This ensures we compile the heavy C++ engine only once, enabling rapid development and consistent binary/behavior states across both players.
2. **Swift Player (`swift-player/`)**: A native macOS SwiftUI player application providing an elegant visual dashboard, parameter tuning, audio monitoring, and MIDI visualization.
3. **Rust Player (`rust-player/`)**: A native, zero-garbage-collection CLI player designed for low-latency, deterministic audio processing, background prompt watch daemons, and WebSocket control over TCP.

---

## 👥 Agent Roles & Ownership

To coordinate work effectively in this monorepo, different AI agents are assigned distinct roles and ownership boundaries:

### 🦀 Rust Player Implementor
- **BEADS_ACTOR Name**: `magenta-rust-implementor`
- **Responsibilities**:
  - Implementation of the Rust FFI bridging using `cxx`.
  - CLI parser ergonomics using `clap`.
  - Native real-time thread safety, MIDI routing, and prompt/file watcher daemons.
  - Rust-side build scripts (`build.rs`) and cargo configs linking against `mrt2-build/`.
- **Target Directories**: `rust-player/`

### 🍏 Swift Player Implementor
- **BEADS_ACTOR Name**: `magenta-swift-implementor`
- **Responsibilities**:
  - Developing the SwiftUI interface, audio engines, and CoreMIDI managers.
  - Writing the C bridge wrapping C++ classes for consumption in Swift.
  - Configuring `Package.swift` to build/link the C bridge against `mrt2-build/`.
- **Target Directories**: `swift-player/`

---

## 📊 Issue Tracking (Beads Workflow)

This project uses **bd (beads)** for distributed issue tracking. Always synchronize your tasks with `bd` before starting work.

Run `bd prime` at the start of your session to load dynamic workflow context, or install git hooks via `bd hooks install` to handle it automatically.

### Quick Reference
* **Find unblocked work**: `bd ready`
* **List open issues**: `bd list --status=open`
* **Claim/Start a task**: `bd update <id> --status=in_progress`
* **Complete/Close tasks**: `bd close <id1> <id2>`
* **Add new task**: `bd create --title="Title" --description="Why/What" --type=task|bug|feature --priority=1-4` (0=highest)

> **Important**: Do *NOT* use `bd edit` as it opens an interactive terminal. Always use `bd update` with CLI flags to modify issues.

---

## 🚨 Session Close Protocol

Before concluding your session, you **must** run the following checklist to ensure the database remains pristine:
- [ ] Run `bd ready` to see remaining work.
- [ ] Run `bd close <id1> <id2> ...` to close all completed tasks in batch.
- [ ] Ensure that no incomplete tasks remain in the `in_progress` state unless they are actively blocked.
