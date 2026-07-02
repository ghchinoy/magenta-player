// magentart_bridge.mm
// ===================
// Objective-C++ implementation of the C bridge declared in magentart_bridge.h.
//
// Must be compiled as Objective-C++ (.mm) because the MLX inference thread
// in RealtimeRunner needs an Objective-C autorelease pool per iteration
// (handled internally in realtime_runner.cpp via magentart::detail::AutoreleasePool).
// Compiling as plain .cpp would link without the ObjC runtime and crash.
//
// The MRT2_BUILD_DIR cmake/SPM variable must be set so the compiler can find:
//   <MRT2_BUILD_DIR>/include/magentart/realtime_runner.h
// and the linker can find:
//   <MRT2_BUILD_DIR>/libmagentart-core.a

#import "magentart_bridge.h"

#include <magentart/realtime_runner.h>

#include <cstring>
#include <string>

using magentart::core::RealtimeRunner;

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

static inline RealtimeRunner* cast(MagentaEngineRef ref) {
    return static_cast<RealtimeRunner*>(ref);
}

// ---------------------------------------------------------------------------
// Lifecycle
// ---------------------------------------------------------------------------

MagentaEngineRef magentart_create(void) {
    return new (std::nothrow) RealtimeRunner();
}

bool magentart_init_assets(MagentaEngineRef engine, const char* resource_dir) {
    if (!engine || !resource_dir) return false;
    return cast(engine)->init_assets(resource_dir);
}

void magentart_destroy(MagentaEngineRef engine) {
    if (!engine) return;
    delete cast(engine);
}

bool magentart_load_model(MagentaEngineRef engine, const char* mlxfn_path) {
    if (!engine || !mlxfn_path) return false;
    return cast(engine)->load_model(mlxfn_path);
}

bool magentart_is_loaded(MagentaEngineRef engine) {
    if (!engine) return false;
    return cast(engine)->is_loaded();
}

void magentart_start(MagentaEngineRef engine) {
    if (!engine) return;
    cast(engine)->start();
}

void magentart_stop(MagentaEngineRef engine) {
    if (!engine) return;
    cast(engine)->stop();
}

void magentart_set_bypass(MagentaEngineRef engine, bool bypass) {
    if (!engine) return;
    cast(engine)->set_bypass(bypass);
}

void magentart_trigger_reset(MagentaEngineRef engine) {
    if (!engine) return;
    cast(engine)->trigger_reset();
}

// ---------------------------------------------------------------------------
// Audio output
// ---------------------------------------------------------------------------

bool magentart_read_audio_stereo(MagentaEngineRef engine,
                                  float* destL,
                                  float* destR,
                                  uint32_t count) {
    if (!engine || !destL || !destR || count == 0) {
        // Zero-fill on bad args so the audio thread never reads garbage
        if (destL) std::memset(destL, 0, count * sizeof(float));
        if (destR) std::memset(destR, 0, count * sizeof(float));
        return false;
    }
    return cast(engine)->read_audio_stereo(destL, destR,
                                           static_cast<std::size_t>(count));
}

// ---------------------------------------------------------------------------
// Sampling parameters
// ---------------------------------------------------------------------------

void magentart_set_temperature(MagentaEngineRef engine, float t) {
    if (!engine) return;
    cast(engine)->set_temperature(t);
}

void magentart_set_top_k(MagentaEngineRef engine, int k) {
    if (!engine) return;
    cast(engine)->set_top_k(k);
}

void magentart_set_cfg_musiccoca(MagentaEngineRef engine, float v) {
    if (!engine) return;
    cast(engine)->set_cfg_musiccoca(v);
}

void magentart_set_cfg_notes(MagentaEngineRef engine, float v) {
    if (!engine) return;
    cast(engine)->set_cfg_notes(v);
}

void magentart_set_cfg_drums(MagentaEngineRef engine, float v) {
    if (!engine) return;
    cast(engine)->set_cfg_drums(v);
}

// ---------------------------------------------------------------------------
// Output control
// ---------------------------------------------------------------------------

void magentart_set_volume_db(MagentaEngineRef engine, float db) {
    if (!engine) return;
    cast(engine)->set_volume_db(db);
}

void magentart_set_mute(MagentaEngineRef engine, bool mute) {
    if (!engine) return;
    cast(engine)->set_mute(mute);
}

// ---------------------------------------------------------------------------
// MIDI
// ---------------------------------------------------------------------------

void magentart_set_midi_gate_enabled(MagentaEngineRef engine, bool enabled) {
    if (!engine) return;
    cast(engine)->set_midi_gate_enabled(enabled);
}

void magentart_set_note_on(MagentaEngineRef engine, int note) {
    if (!engine) return;
    cast(engine)->set_note_on(note);
}

void magentart_set_note_off(MagentaEngineRef engine, int note) {
    if (!engine) return;
    cast(engine)->set_note_off(note);
}

// ---------------------------------------------------------------------------
// Prompts
// ---------------------------------------------------------------------------

void magentart_set_text_prompt(MagentaEngineRef engine, const char* text) {
    if (!engine) return;
    cast(engine)->set_text_prompt(std::string(text ? text : ""));
}

// ---------------------------------------------------------------------------
// Buffer size
// ---------------------------------------------------------------------------

void magentart_set_buffer_size(MagentaEngineRef engine, uint32_t samples) {
    if (!engine) return;
    cast(engine)->set_buffer_size(static_cast<std::size_t>(samples));
}

// ---------------------------------------------------------------------------
// Metrics
// ---------------------------------------------------------------------------

void magentart_reset_dropped_frames(MagentaEngineRef engine) {
    if (!engine) return;
    cast(engine)->reset_dropped_frames();
}

MagentaMetrics magentart_get_metrics(MagentaEngineRef engine) {
    MagentaMetrics out{};
    if (!engine) return out;
    auto m = cast(engine)->get_metrics();
    out.transformer_ms   = m.transformer_ms;
    out.total_ms         = m.total_ms;
    out.buffer_available = m.buffer_available;
    out.buffer_capacity  = m.buffer_capacity;
    out.transport_flags  = m.transport_flags;
    out.dropped_frames   = m.dropped_frames;
    return out;
}
