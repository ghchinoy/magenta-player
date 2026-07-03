// Header file declaring the bridge wrappers.
// This is used by 'cxx' to generate compile-time bidirectional bindings between Rust and the C++ magentart core.

#pragma once
#include <memory>
#include <string>
#include <vector>
#include <magentart/realtime_runner.h>
#include "rust/cxx.h"

class RealtimeRunnerBridge {
public:
    RealtimeRunnerBridge() : runner_(std::make_unique<magentart::core::RealtimeRunner>()) {}

    bool load_model(rust::Str path) {
        std::string path_str(path.data(), path.size());
        return runner_->load_model(path_str.c_str());
    }

    /// Loads the MusicCoCa tokenizer, text-encoder, mapper, and quantizer
    /// TFLite interpreters from the resources directory. This MUST be called
    /// before set_prompt() will have any effect: without it, tokenizer_,
    /// text_encoder_interpreter_, and quantizer_interpreter_ remain null and
    /// fetch_musiccoca_tokens() silently fails (text_encoder_status_ = 3),
    /// leaving musiccoca_tokens_ permanently at its hardcoded
    /// kDefaultMusicCoCaTokensPiano value.
    bool init_assets(rust::Str resource_dir) {
        std::string dir_str(resource_dir.data(), resource_dir.size());
        return runner_->init_assets(dir_str.c_str());
    }

    void set_prompt(rust::Str prompt) {
        std::string prompt_str(prompt.data(), prompt.size());
        std::vector<std::string> prompts = { prompt_str };
        std::vector<float> weights = { 1.0f };
        runner_->set_text_prompts(prompts, weights);
        
        // Explicitly set blend weights for the CLI player so that our custom
        // prompt (slot 0) has 100% weight, and slots 1, 2, and 3 are completely zeroed out.
        // This overrides RealtimeRunner's default 2D surface mode (which blends 50% of
        // slot 0 and 50% of slot 1, causing the custom prompt to be diluted with 50% of 
        // the default empty/piano prompt).
        runner_->set_blend_weight(0, 1.0f);
        runner_->set_blend_weight(1, 0.0f);
        runner_->set_blend_weight(2, 0.0f);
        runner_->set_blend_weight(3, 0.0f);
    }

    void set_temperature(float temp) {
        runner_->set_temperature(temp);
    }

    void set_top_k(uint32_t k) {
        runner_->set_top_k(k);
    }

    void set_midi_gate(bool enabled) {
        runner_->set_midi_gate_enabled(enabled);
    }

    void set_buffer_size(size_t cap) {
        runner_->set_buffer_size(cap);
    }

    // CFG (classifier-free guidance) weights control how strongly each
    // conditioning signal steers generation. Higher = more strongly
    // adherent to that signal, at some cost to naturalness/diversity.
    // Factory defaults: text=3.0, notes=5.0, drums=1.0.
    void set_cfg_text(float v) {
        runner_->set_cfg_musiccoca(v);
    }

    void set_cfg_notes(float v) {
        runner_->set_cfg_notes(v);
    }

    void set_cfg_drums(float v) {
        runner_->set_cfg_drums(v);
    }

    void set_drumless(bool on) {
        runner_->set_drumless(on);
    }

    void set_volume_db(float v) {
        runner_->set_volume_db(v);
    }

    void toggle_play(bool playing) const {
        auto* r = const_cast<magentart::core::RealtimeRunner*>(runner_.get());
        r->set_bypass(!playing);
        if (playing) {
            r->trigger_reset();
        }
    }

    /// Returns the async MusicCoCa quantizer status for prompt slot encoding:
    /// 0 = idle, 1 = fetching/encoding, 2 = success (tokens ready), 3 = error.
    /// Used to poll until our custom prompt has replaced the engine's
    /// hardcoded default piano tokens (see kDefaultMusicCoCaTokensPiano in
    /// mlx_engine.h) before unmuting audio output.
    int32_t get_quantizer_status() const {
        return static_cast<int32_t>(runner_->get_quantizer_status());
    }

    bool read_audio_stereo(rust::Slice<float> dest_l, rust::Slice<float> dest_r) const {
        // Const_cast is safe here because read_audio_stereo has atomic operations on lock-free ring buffers
        auto* r = const_cast<magentart::core::RealtimeRunner*>(runner_.get());
        return r->read_audio_stereo(dest_l.data(), dest_r.data(), dest_l.size(), false);
    }

    rust::String read_metrics() const {
        auto m = runner_->get_metrics();
        // Return structured metrics as JSON to easily parse on the Rust side
        std::string s = "{\"transformer_ms\":" + std::to_string(m.transformer_ms) + 
                       ",\"dropped_frames\":" + std::to_string(m.dropped_frames) + "}";
        return rust::String(s);
    }

    // Recording controls: the engine maintains an internal circular buffer of
    // ALL generated audio automatically once start_recording() is called --
    // no manual sample-capture loop needed on our side. const + const_cast
    // here to match toggle_play()/read_audio_stereo() so these are callable
    // through the shared (immutable) Arc<RealtimeRunnerBridge> used by the
    // CPAL audio thread, from the main thread, without needing Pin<&mut>.
    void start_recording() const {
        const_cast<magentart::core::RealtimeRunner*>(runner_.get())->start_recording();
    }

    void stop_recording() const {
        const_cast<magentart::core::RealtimeRunner*>(runner_.get())->stop_recording();
    }

    size_t get_recorded_sample_count() const {
        return runner_->get_recorded_sample_count();
    }

    bool get_recorded_audio(size_t start_idx, rust::Slice<float> dest_l, rust::Slice<float> dest_r) const {
        return runner_->get_recorded_audio(dest_l.data(), dest_r.data(), start_idx, dest_l.size());
    }

private:
    std::unique_ptr<magentart::core::RealtimeRunner> runner_;
};

inline std::unique_ptr<RealtimeRunnerBridge> create_runner() {
    return std::make_unique<RealtimeRunnerBridge>();
}
