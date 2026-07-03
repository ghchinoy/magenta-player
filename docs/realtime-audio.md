# Real-Time Audio on macOS — Lessons Learned

## Ring Buffer Sizing

MRT2 generates audio at **25 Hz** — one inference frame of **1920 samples**
every 40 ms at 48 kHz. The ring buffer's default `virtual_capacity` is 2048
samples, leaving:

```
headroom = 2048 − 1920 = 128 samples ≈ 2.7 ms
```

Metal GPU scheduling jitter easily exceeds 2.7 ms, causing underruns.
`read_audio_stereo(blocking=false)` zero-pads on underrun — and alternating
audio/silence sounds like **wobble or warping**, not the obvious clicks you
might expect from a dropout.

**Fix:** set `virtual_capacity` to **8192 samples (~170 ms = 4 frames)** before
calling `start()`. This is the physical maximum and absorbs GPU scheduling
variance on both `mrt2_small` and `mrt2_base`.

```swift
magentart_set_buffer_size(ref, 8192)
magentart_start(ref)
```

`mrt2_base` inference on borderline hardware (M1 Pro) can consume 50–80 ms per
frame. With only 4096 samples (~85 ms = 2 frames) of headroom, variance in
Metal kernel scheduling still causes occasional underruns. 8192 eliminates
them. For a generative music tool, the extra ~85 ms of latency is
imperceptible.

**Measured (Rust/CPAL, Apple M5 base tier, 32GB, magenta-player-iii.6):** `mrt2_small`
is rock-stable at both 4096 and 8192 — transformer 13–23 ms/frame, dropped_frames
pins at the single startup-priming frame and never grows again at 8192. `mrt2_base`
measured 57–74 ms/frame (vs the 40 ms budget) at **both** buffer sizes, with
dropped_frames growing linearly and unboundedly in both cases (~55–70/2s-tick at
4096, ~76–87/2s-tick at 8192) — confirming this specific machine is genuinely
throughput-bound for `mrt2_base`, not jitter-bound; 8192 changed nothing for it, as
the docs' own caveat predicts. Notably this is a **base-tier M5**, newer than the
M1 Pro reference case above but without Pro/Max-class memory bandwidth — the
"M1 Pro and above" real-time guidance for `mrt2_base` should probably be read as
"Pro/Max tier and above, any generation" rather than a strict chip-generation cutoff.
Kept 8192 as the new default regardless: strictly better for `mrt2_small`, neutral
(no worse) for `mrt2_base`.

## Startup Priming

When `start()` is called the ring buffer is **empty**. `AVAudioSourceNode`
fires its render callback within ~10 ms; the first reads are all zeros.
Zero-padded startup produces the same wobble artifact as underruns.

**Fix:** keep the engine in bypass for two frame durations (80 ms) after
`start()`, then release bypass. The inference thread fills the buffer during
that window, so the first audio pull gets real content.

```swift
magentart_set_bypass(ref, true)
magentart_start(ref)
Task {
    try? await Task.sleep(nanoseconds: 80_000_000)   // 2 × 40 ms frames
    await MainActor.run {
        guard isStillPlaying else { return }
        magentart_set_bypass(ref, false)
    }
}
```

Do **not** set `blocking = true` in the render callback — it is documented
for offline rendering only and will cause real-time thread violations.

## AVAudioSourceNode Format

Use a **non-interleaved** `AVAudioFormat` matching the engine's output:

```swift
let format = AVAudioFormat(standardFormatWithSampleRate: 48_000, channels: 2)!
```

`standardFormatWithSampleRate` produces 32-bit float, non-interleaved, which
matches `read_audio_stereo(float* destL, float* destR, ...)` directly. Do not
use an interleaved format — you will need to de-interleave manually or the
stereo image will be corrupted.

## Output Sample Rate: 48 kHz vs 44.1 kHz

MRT2 produces audio strictly at **48,000 Hz**. Many external Bluetooth and
network speakers (e.g. Sonos Roam) are hardware-locked to **44,100 Hz**.

### Swift player (`AVAudioEngine`)

`AVAudioEngine` handles sample rate conversion **automatically and
transparently** when the hardware output rate differs from the format you
requested. If you configure the source node at 48 kHz and the device runs at
44.1 kHz, `AVAudioEngine` resamples internally — no pitch shift, no manual
intervention required. This is not a concern for the Swift player.

For the cleanest signal path (zero SRC), open macOS **Audio MIDI Setup** and
set the output device to exactly **48,000 Hz** stereo. This is optional but
eliminates any resampling artefacts entirely.

### Rust player (CPAL / direct device access)

CPAL writes directly to whatever sample rate the hardware device exposes.
If the device reports 44.1 kHz and you write 48 kHz content without resampling:

- **Pitch and speed shift**: 48 kHz frames played back at 44.1 kHz run at
  `44100 / 48000 = 91.875%` speed — a noticeable flat pitch shift.
- **Interpolation artefacts**: On-the-fly resampling without a proper
  anti-aliasing filter produces aliasing and filter ringing, which can sound
  like a subtle warble or harmonic distortion (distinct from a ring-buffer
  dropout).

**Fixes for the Rust player:**
- Query the device's preferred sample rate via CPAL before starting the
  stream; warn the user if it differs from 48 kHz.
- **Resolved (magenta-player-3vy.2):** Implemented a high-fidelity, lock-free,
  zero-allocation linear resampler with boundary-preserved sample memory directly
  in the CPAL callback. When a 44.1 kHz device (like Sonos Roam) is active, it
  calculates the dynamic input frames needed, pulls them via `read_audio_stereo`,
  interpolates with phase continuity across buffer boundaries, and writes stereo
  output. This completely eliminates pitch-shifting, speed drops, and boundary
  clicks with negligible CPU overhead.

## TimelineView + Canvas: Always Capture `context.date`

`TimelineView` ticks on schedule, but `Canvas` is a reference-type-aware
SwiftUI primitive — it only re-executes its drawing closure if SwiftUI detects
a change in its captured inputs. If all your inputs are reference types
(e.g. a ring buffer `final class`), SwiftUI sees the same pointer on every
tick and **caches the Canvas output**, producing a frozen display even though
the underlying data is updating on the audio thread at 30+ fps.

**Fix:** explicitly capture `context.date` inside the Canvas closure:

```swift
TimelineView(.animation(minimumInterval: 1.0 / 30)) { context in
    Canvas { ctx, size in
        _ = context.date   // ← this one line forces a redraw on every tick
        let (snapL, snapR) = myRingBuffer.snapshot()
        // ... draw ...
    }
}
```

`context.date` is a `Date` that changes on every tick. Capturing it inside
the Canvas closure establishes a SwiftUI dependency on the timeline schedule,
bypassing the reference-equality optimisation that otherwise freezes the view.
This applies to any animated Canvas driven by mutable reference-type data —
audio visualisers, live meters, scrolling plots, etc.

## Bypass-as-Pause: Implementing Pause Without Stopping Inference

`set_bypass(true)` silences the output while the inference thread keeps
running. This is the correct implementation of Pause in a generative player —
it preserves the full audio context window and allows seamless resume.

**Do not call `stop()` on Pause.** Stopping the inference thread means:
- Ring buffer drains
- Inference state is frozen mid-frame
- Resume requires a full `trigger_reset()` + `start()` + 80 ms priming delay

```
// CORRECT: Pause
set_bypass(true)          // audio → silence; inference thread → still running
// ... user decides to resume ...
set_bypass(false)         // audio resumes instantly; no priming, no click

// WRONG: "Pause" via stop
stop()                    // inference halts, ring buffer drains
// ... user resumes ...
trigger_reset()           // required to clear stale state
start()                   // 80 ms wobble-free priming needed → bad UX
```

**For CPAL (Rust):** bypass means writing zeros to the CPAL output callback
while letting the MRT2 inference thread run freely on its own OS thread. Do
not use `thread::park` or `Condvar::wait` on the inference thread — it needs
to keep filling the ring buffer even while audio is silent.

## Audio Engine Setup Must Happen Before Playback

`setupAudioEngine()` is **not** called automatically. If you create a
`PlayerManager` without wiring its `setupAudioEngine()` call (e.g. from
`onAppear`), the `AVAudioEngine` never starts and the ring buffer is never
drained — complete silence with no error.

```swift
.onAppear {
    playerManager.setupAudioEngine()
}
```

## Level Meters: Use RMS, Display in dBFS

Computing RMS in the audio render block is fine (it's just a loop over floats).
Display the result as dBFS (`20 * log10(rms)`) not as raw amplitude — raw
values like `0.45` are meaningless to users.

```swift
var sum: Float = 0
for i in 0..<n { sum += pL[i] * pL[i] }
let rms = sqrtf(sum / Float(n))
let dBFS = rms > 0 ? 20 * log10(rms) : -Float.infinity
```

## init_assets() Is Required Before Prompts Work — Silent Failure Otherwise

`RealtimeRunner::set_text_prompts()` / `set_prompt()` only *schedules* a
background encode via `fetch_musiccoca_tokens()`. That function bails out
immediately — silently — if the tokenizer, text-encoder, or quantizer TFLite
interpreters aren't loaded:

```cpp
if (!tokenizer_ || !text_encoder_interpreter_ || !quantizer_interpreter_ || ...) {
    text_encoder_status_ = 3;  // error
    quantizer_status_ = 3;     // error
    return;
}
```

Those interpreters are populated by **`init_assets(resource_dir)`** /
**`load_musiccoca_model(resource_dir, subfolder)`** — a separate lifecycle
call from `load_model()` (which only loads the MLX transformer weights).
**Nothing calls `init_assets` for you automatically.**

If you skip it: `set_prompt()` appears to succeed (no exception, no return
value to check), but `musiccoca_tokens_` never leaves its hardcoded
`kDefaultMusicCoCaTokensPiano` value (see `mlx_engine.cpp`), no matter what
blend weights you set or what order you call things in. **You will hear the
default piano prompt forever**, regardless of the prompt text you send.

**Fix:** call `init_assets(resources_dir)` once, right after constructing the
runner and *before* the first `set_prompt()` / `load_model()` call. Check its
return value — a `false` means your resources path is wrong.

```cpp
runner->init_assets(resources_dir);   // loads tokenizer/text-encoder/quantizer
runner->set_prompt("lofi jazz piano");
runner->load_model(mlxfn_path);
```

**Verifying it worked:** watch stdout for `[MagentaRT] Combined Prompt (N)
tokens: ...` with token values that change per-prompt. If you only ever see
the model's default piano tokens, `init_assets` did not succeed.

**Closing the last race:** even with `init_assets` done, encoding is
asynchronous (`fetch_musiccoca_tokens` runs on a detached thread). Poll
`get_quantizer_status()` (0=idle, 1=fetching, 2=success, 3=error) after
`set_prompt()`/`load_model()` and don't unmute audio / open the output stream
until it reports `2`, to guarantee zero default-prompt bleed on startup.

## Threading Model

| Thread | What you can call |
| :--- | :--- |
| Audio render (real-time) | `magentart_read_audio_stereo` only. Lock-free. Never allocate, never take a mutex. |
| Background / `Task.detached` | `magentart_load_model` (blocking, compiles MLX graph), `magentart_init_assets` (blocking) |
| Any thread | Parameter setters (`set_temperature`, `set_top_k`, etc.) — all atomic |
| Main actor | UI state updates, `start()`, `stop()`, `set_bypass()` |

`LevelTracker` (the struct passing RMS from the render block to the UI timer)
uses natural atomicity of aligned `Float` stores on Apple Silicon — acceptable
for a meter where a stale frame is harmless. For anything correctness-critical,
use a proper atomic or lock.
