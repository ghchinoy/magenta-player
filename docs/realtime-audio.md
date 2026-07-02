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

**Fix:** set `virtual_capacity` to at least **4096 samples (~85 ms = 2 frames)**
before calling `start()`. This absorbs GPU scheduling variance on both
`mrt2_small` and `mrt2_base`.

```swift
magentart_set_buffer_size(ref, 4096)
magentart_start(ref)
```

Maximum physical capacity is `RingBuffer::kCapacity = 8192` samples (~170 ms).
Higher values increase latency; 4096 is the stable sweet spot for interactive
use.

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
