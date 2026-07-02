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

**Diagnosing frame drops:** watch the `dropped_frames` counter in the engine
metrics — it increments every time `read_audio_stereo` zero-pads because the
ring buffer was empty. A growing counter means inference is slower than
real-time and no buffer size will fix it; the hardware simply cannot sustain
25 Hz inference for that model. The 8192-sample buffer (~170 ms) eliminates
drops caused by *scheduling jitter* on capable hardware, but cannot compensate
for hardware that is genuinely too slow.

If `dropped_frames` grows continuously with `mrt2_base`, switch to
`mrt2_small`, which runs in real-time on every Apple Silicon Mac.

## Playback State Machine: bypass ≠ stop

There are three distinct playback states with very different semantics:

| State | API | Inference thread | Audio context | Resume cost |
| :--- | :--- | :--- | :--- | :--- |
| **Playing** | `start()` + `set_bypass(false)` | Running | Accumulating | — |
| **Paused** | `set_bypass(true)` only | **Still running** | **Fully preserved** | Instant — just `set_bypass(false)` |
| **Stopped** | `stop()` + `set_bypass(true)` | Halted | Stale (not cleared) | Full restart + priming delay |

The most important lesson: **`set_bypass(true)` is Pause, not Mute.**

When bypassed, the inference thread continues generating at 25 Hz and filling
the ring buffer. Audio output is simply suppressed. When bypass is released,
generation resumes from the exact musical moment — no click, no priming delay,
no context loss. This is how a natural Pause/Resume works.

`stop()` halts the inference thread. Restarting costs the full 80 ms priming
delay plus a `trigger_reset()` to clear any stale state. Use `stop()` only
when the user truly intends to end the session.

```
// Play — fresh start
trigger_reset()          // clear any prior context
start()
set_bypass(true)         // silent during 80 ms prime
sleep(80ms)
set_bypass(false)        // audio flows, buffer is full

// Pause — context intact
set_bypass(true)         // inference keeps running, ring buffer keeps filling

// Resume — seamless
set_bypass(false)        // instant, no priming needed, musical continuity preserved

// Stop
stop()
set_bypass(true)
```

**Rust / CPAL note:** the same three states apply. "Pause" in a CPAL callback
means writing zeros to the output buffer while letting the inference thread
keep running on its own thread — do not call `stop()` on pause.

## Reset Variants: User vs Transport

There are two reset calls with subtly different behaviour:

```cpp
trigger_reset()            // user-initiated: ALWAYS fires, even after prefill
trigger_transport_reset()  // transport/DAW: suppressed once after a prefill
```

Use `trigger_reset()` for all UI-driven resets (Play button, Reset button).
`trigger_transport_reset()` is for DAW host integration only — it respects
the post-prefill grace window that lets a fresh prefill survive a DAW rewind.

Always call `trigger_reset()` at the start of a new play session:

```swift
trigger_reset()   // clear stale context
start()
```

Without it the engine may resume from a half-accumulated audio context, producing
jarring harmonic discontinuities at playback start.

**Also reset `dropped_frames` at each play start** — the counter is cumulative
since engine creation. Resetting it gives per-session underrun counts:

```swift
reset_dropped_frames()   // zero the counter; new session starts clean
trigger_reset()
start()
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

## State Persistence: save_state, load_state, prefill_state

The engine has two complementary ways to checkpoint and restore generation:

### `save_state(path)` / `load_state(path)`

Serialises the transformer's full KV cache (the audio context window) to a
`.safetensors` file. Loading restores the exact internal state — generation
**continues from the same musical moment** as if no time passed.

```cpp
runner->save_state("/path/to/seeds/2026-07-02/state.safetensors");
// later:
runner->load_state("/path/to/seeds/2026-07-02/state.safetensors");
runner->start();   // resumes from the checkpointed moment
```

After `load_state`, subsequent `trigger_reset()` calls return to the loaded
state (not factory silence) until `reset_to_factory()` is called explicitly.

### `prefill_state(samples, count)` / `prefill_silence(duration_frames)`

Loads PCM audio into the context window and checkpoints it. Unlike `load_state`,
this does **not** restore a saved session — it seeds the model with audio so
that generation follows from it stylistically.

```cpp
// Seed from a WAV you recorded
runner->prefill_state(pcm_samples, num_samples);
runner->start();   // generates music that "continues" from that audio

// Or: seed from silence (fills context with cached silent tokens)
runner->prefill_silence(550);   // 550 frames = ~22 s = full attention window
runner->start();   // cleanest possible cold start
```

The engine trims ~1 s from each end of prefill audio to remove boundary
artefacts. Prefill checkpoints just like `load_state` — `trigger_reset()`
returns to the prefilled context until `reset_to_factory()` clears it.

### `reset_to_factory()`

Undoes any checkpoint (from `prefill_state`, `prefill_silence`, or
`load_state`) and restores the model's original factory initial state. Use
this when you want a completely clean slate independent of any prior session.

### Practical pattern: Memory Banks

```
// Save a seed while something sounds great:
runner->stop()
runner->save_state(seed_path + "/state.safetensors")
record WAV from get_recorded_audio()
save prompt + params to JSON
runner->start() with trigger_reset() to resume

// Reload later:
runner->load_state(seed_path + "/state.safetensors")
apply_saved_params()
runner->start()   // exact continuation, same musical moment
```

## Recording Buffer

The engine maintains an internal circular recording buffer of all generated audio:

```cpp
runner->start_recording();                          // begin capturing
// ... generation runs ...
runner->stop_recording();
size_t count = runner->get_recorded_sample_count(); // total stereo samples
// retrieve the last N seconds:
runner->get_recorded_audio(dest_L, dest_R, start_idx, count);

// waveform thumbnail for UI (returns peak amplitudes, one per bucket):
auto peaks = runner->get_waveform_peaks(200);       // 200-bucket reduction
```

Use `get_waveform_peaks` to draw a visual thumbnail of a saved seed without
decoding the full WAV — fast enough to call in a list view at scroll time.

## Resources Path from `mrt models init` vs App Bundle

In a future `.app` bundle deployment, the SpectroStream and MusicCoCa TFLite
models will need to be bundled with the app or downloaded on first launch.
Currently they live in `~/Documents/Magenta/magenta-rt-v2/resources/` (the
`mrt models init` download path). When packaging for distribution, plan for
either bundling these resources (large, ~500 MB) or an in-app download flow.
