# MRT2 Engine Integration — Lessons Learned

## `init_assets` Is Required Before Any Audio

`load_model` loads only the MLX transformer weights. The SpectroStream codec
(audio tokens → PCM samples) and MusicCoCa text encoder are separate TFLite
models loaded by `init_assets`. Without it:

- `load_model` returns `true` (the transformer loaded fine)
- `generate_frame` runs but cannot decode tokens to audio
- The ring buffer fills with zeros → **complete silence**

Always call `init_assets` before `load_model`:

```swift
magentart_init_assets(ref, resourcesDir)   // must succeed first
magentart_load_model(ref, mlxfnPath)
```

## Resources and Model Files Are Downloaded Separately

`mrt models download <name>` downloads the transformer weights (`.mlxfn`).
`mrt models init` downloads the shared codec assets (`musiccoca/`,
`spectrostream/`). Both are required; neither implies the other.

```
~/Documents/Magenta/magenta-rt-v2/
├── mrt2_small.mlxfn              ← from: mrt models download mrt2_small
├── mrt2_small_state.safetensors
└── resources/                    ← from: mrt models init  (shared, one-time)
    ├── musiccoca/
    └── spectrostream/
```

The resources path is a sibling of the model file, not inside the model
directory. Derive it by walking up from the `.mlxfn` file looking for a
`resources/` directory rather than hardcoding a level count — the user may
have placed the model file at a different nesting depth.

## 25 Hz Inference Cadence

The engine generates audio at **25 frames per second**, one frame = 1920
samples at 48 kHz = 40 ms per frame. All timing calculations should use this
as the base unit:

| Value | Samples | Time |
| :--- | ---: | ---: |
| 1 inference frame | 1920 | 40 ms |
| Default ring buffer | 2048 | 42.7 ms |
| Recommended buffer | 4096 | 85.3 ms |
| Max ring buffer | 8192 | 170.7 ms |

## Text Prompt Is the Primary Creative Control

The engine generates unconditioned (random/average) music if no text prompt is
set. `set_text_prompt` routes through MusicCoCa to embed a style vector into
the generation. This is the most important user-facing parameter — without it,
the app is musically unconstrained.

```swift
magentart_set_text_prompt(ref, "jazz piano trio")
```

Prompts are accepted live during playback — no stop/restart required.

## Two Model Sizes, No Others

Only `mrt2_small` (230M) and `mrt2_base` (2.4B) exist. The name `mrt2_large`
appears in source comments as an alias for `mrt2_base` — it is not a third
model. Hardware requirements:

| Model | Real-time on |
| :--- | :--- |
| `mrt2_small` | Any Apple Silicon (M1 Air and above) |
| `mrt2_base` | M1 Pro / Max and above |

## `set_bypass` Not `stop` for Muting

`stop()` halts the inference thread entirely — restarting incurs a full reset
and priming delay. For momentary silence (mute, pause), prefer:

```swift
magentart_set_bypass(ref, true)   // silence audio, inference keeps running
magentart_set_bypass(ref, false)  // resume instantly, no priming needed
```

Use `stop()` only when the user explicitly stops generation and will not
resume immediately.

## `trigger_reset` on Play Start

Call `trigger_reset()` when starting playback. Without it, the engine resumes
from whatever state it was left in (potentially mid-way through an old
generation), which can produce jarring audio discontinuities. The reset
envelope fades in the next frame to avoid a click:

```swift
magentart_trigger_reset(ref)
magentart_start(ref)
```

## `load_model` Blocks for Several Seconds

`load_model` compiles the MLX computation graph for the target Apple Silicon
chip — this takes 5–15 seconds on first load. Always dispatch to a background
thread and show a loading indicator:

```swift
state.modelName = "Loading \(name)…"
Task.detached(priority: .userInitiated) {
    let ok = magentart_load_model(ref, path)
    await MainActor.run { ... }
}
```

Calling it on the main thread will freeze the UI.

## Resources Path from `mrt models init` vs App Bundle

In a future `.app` bundle deployment, the SpectroStream and MusicCoCa TFLite
models will need to be bundled with the app or downloaded on first launch.
Currently they live in `~/Documents/Magenta/magenta-rt-v2/resources/` (the
`mrt models init` download path). When packaging for distribution, plan for
either bundling these resources (large, ~500 MB) or an in-app download flow.
