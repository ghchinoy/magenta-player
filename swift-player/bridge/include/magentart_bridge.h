// magentart_bridge.h
// ==================
// C interface over magentart::core::RealtimeRunner for Swift FFI.
//
// Swift cannot call C++ directly, so this header exposes a flat C API using
// an opaque handle (MagentaEngineRef). The implementation lives in
// magentart_bridge.mm (Objective-C++ so it can host the MLX autorelease pool).
//
// Include this file in the Swift bridging header:
//   #import "magentart_bridge.h"
//
// All functions are safe to call from the main/UI thread unless noted.

#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// ---------------------------------------------------------------------------
// Opaque engine handle
// ---------------------------------------------------------------------------

/// Opaque pointer to a heap-allocated magentart::core::RealtimeRunner.
/// Created by magentart_create(), destroyed by magentart_destroy().
typedef void* MagentaEngineRef;

// ---------------------------------------------------------------------------
// Metrics snapshot
// ---------------------------------------------------------------------------

/// Mirrors magentart::core::EngineMetrics. Populated by the inference thread;
/// safe to read from the UI thread via magentart_get_metrics().
typedef struct {
    float    transformer_ms;    ///< Last inference step duration (ms)
    float    total_ms;          ///< Total frame processing time (ms)
    size_t   buffer_available;  ///< Stereo samples available in ring buffer
    size_t   buffer_capacity;   ///< Ring buffer total capacity in samples
    int      transport_flags;   ///< -1 uninit, -2 no host blocks, -3 block NO
    uint64_t dropped_frames;    ///< Cumulative real-time underruns since reset
} MagentaMetrics;

// ---------------------------------------------------------------------------
// Lifecycle  (UI/controller thread)
// ---------------------------------------------------------------------------

/// Allocate and return a new engine. Returns NULL on allocation failure.
MagentaEngineRef magentart_create(void);

/// Load shared codec and encoder assets (SpectroStream TFLite, MusicCoCa TFLite)
/// from resource_dir. Must be called before any audio is generated.
/// resource_dir should be ~/Documents/Magenta/magenta-rt-v2/resources/
/// (the directory populated by `mrt models init`).
/// Returns true on success.
bool magentart_init_assets(MagentaEngineRef engine, const char* resource_dir);

/// Destroy and free an engine. Sets any cached references in Swift to nil
/// before calling. Stops the inference thread internally.
void magentart_destroy(MagentaEngineRef engine);

/// Load model weights from an .mlxfn path. Returns true on success.
/// Blocks until load is complete — call from a background thread or
/// DispatchQueue.global() to keep the UI responsive.
bool magentart_load_model(MagentaEngineRef engine, const char* mlxfn_path);

/// Returns true after a successful magentart_load_model().
bool magentart_is_loaded(MagentaEngineRef engine);

/// Start the 25 Hz inference loop. Engine must be loaded first.
void magentart_start(MagentaEngineRef engine);

/// Stop the inference loop. Audio reads will zero-pad after the ring buffer
/// drains.
void magentart_stop(MagentaEngineRef engine);

/// Bypass: when true, read_audio_stereo outputs silence without consuming
/// inference frames.
void magentart_set_bypass(MagentaEngineRef engine, bool bypass);

/// Rising-edge user-initiated reset. Fades in the next frame to avoid a click.
void magentart_trigger_reset(MagentaEngineRef engine);

// ---------------------------------------------------------------------------
// Audio output  (audio thread — lock-free)
// ---------------------------------------------------------------------------

/// Pull `count` stereo 32-bit float samples at 48 kHz into destL / destR.
/// Returns false if the ring buffer underran (output is zero-padded in that
/// case — never leaves destL/destR uninitialised).
///
/// Call from AVAudioSourceNode's render block. Never pass blocking=true here.
bool magentart_read_audio_stereo(MagentaEngineRef engine,
                                  float* destL,
                                  float* destR,
                                  uint32_t count);

// ---------------------------------------------------------------------------
// Sampling parameters  (atomic — any thread)
// ---------------------------------------------------------------------------

void magentart_set_temperature(MagentaEngineRef engine, float t);
void magentart_set_top_k(MagentaEngineRef engine, int k);

// ---------------------------------------------------------------------------
// Output control  (atomic — any thread)
// ---------------------------------------------------------------------------

void magentart_set_volume_db(MagentaEngineRef engine, float db);
void magentart_set_mute(MagentaEngineRef engine, bool mute);

// ---------------------------------------------------------------------------
// MIDI  (atomic — any thread)
// ---------------------------------------------------------------------------

/// Enable/disable the MIDI-gate envelope (attenuates output when no notes held).
void magentart_set_midi_gate_enabled(MagentaEngineRef engine, bool enabled);

/// Note-on: MIDI note number 0–127.
void magentart_set_note_on(MagentaEngineRef engine, int note);

/// Note-off: MIDI note number 0–127.
void magentart_set_note_off(MagentaEngineRef engine, int note);

// ---------------------------------------------------------------------------
// Prompts  (UI thread)
// ---------------------------------------------------------------------------

/// Set a single free-text style prompt (replaces slot 0).
void magentart_set_text_prompt(MagentaEngineRef engine, const char* text);

// ---------------------------------------------------------------------------
// Buffer size  (UI thread)
// ---------------------------------------------------------------------------

/// Set the ring buffer's effective write-ahead capacity in samples.
/// Default is 2048 (~42 ms), which leaves only ~2.7 ms of headroom above
/// one 1920-sample inference frame — too tight for Metal scheduling jitter.
/// Recommended: 4096 (~85 ms, 2 frames) for stable real-time playback.
/// Maximum is RingBuffer::kCapacity = 8192 samples (~170 ms).
void magentart_set_buffer_size(MagentaEngineRef engine, uint32_t samples);

// ---------------------------------------------------------------------------
// Metrics  (UI thread)
// ---------------------------------------------------------------------------

/// Snapshot current engine metrics. Safe to call frequently (e.g. 10 Hz).
MagentaMetrics magentart_get_metrics(MagentaEngineRef engine);

#ifdef __cplusplus
}
#endif
