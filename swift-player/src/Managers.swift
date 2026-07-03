import Foundation
import SwiftUI
import AVFoundation
import CoreMIDI

// MARK: - Player errors

enum PlayerError: LocalizedError {
    case engineCreationFailed
    case assetsNotFound(resourcesDir: String)
    case modelLoadFailed(path: String)
    case notLoaded

    var errorDescription: String? {
        switch self {
        case .engineCreationFailed:
            return "Failed to create the Magenta inference engine."
        case .assetsNotFound(let dir):
            return """
            Shared model resources not found at:
              \(dir)
            Run the following command to download them:
              mrt2-build/.venv/bin/mrt models init
            """
        case .modelLoadFailed(let p):
            return "Failed to load model at \(URL(fileURLWithPath: p).lastPathComponent)."
        case .notLoaded:
            return "No model loaded. Use File > Load Model… before playing."
        }
    }
}

// MARK: - Engine handle

/// Wraps the opaque C MagentaEngineRef so it can be safely captured across
/// isolation boundaries. All bridge functions that accept this handle are
/// documented thread-safe in magentart_bridge.h.
private final class EngineHandle: @unchecked Sendable {
    let ref: MagentaEngineRef

    init?() {
        let r = magentart_create()
        guard r != nil else { return nil }
        ref = r!
    }

    deinit {
        magentart_stop(ref)
        magentart_destroy(ref)
    }
}

// MARK: - Level tracker

/// Holds the most-recent per-channel RMS level.
/// Written by the audio render thread, read by the metrics timer.
/// Aligned 32-bit float stores on Apple Silicon are single-instruction,
/// making this safe for metering (a stale frame is acceptable).
final class LevelTracker: @unchecked Sendable {
    var left:  Float = 0
    var right: Float = 0
}

// MARK: - Waveform buffer

/// Lock-free circular buffer of peak-amplitude pairs written by the audio
/// render thread, read by the Canvas in WaveformView at 30 fps.
///
/// Each render callback pushes `samplesPerCallback` pairs (one peak per
/// `chunkSize`-sample chunk). Natural Apple Silicon float-store atomicity
/// is sufficient — a stale or torn waveform frame is visually imperceptible.
final class WaveformBuffer: @unchecked Sendable {
    static let capacity        = 512   // display columns of rolling history
    static let samplesPerPush  = 8     // peaks pushed per render callback
    // chunk = frameCount / samplesPerPush (≈64 samples @ 512-frame buffer)

    // Written by audio thread, read by Canvas — no locking needed for display
    private(set) var peaksL = [Float](repeating: 0, count: capacity)
    private(set) var peaksR = [Float](repeating: 0, count: capacity)
    // `head` is the NEXT write position (oldest visible sample is at head,
    // newest is at head-1 mod capacity)
    private(set) var head   = 0

    /// Push one peak pair. Call from the audio render thread.
    func push(l: Float, r: Float) {
        peaksL[head] = l
        peaksR[head] = r
        head = (head + 1) % Self.capacity
    }

    /// Snapshot the buffer into two flat arrays ordered oldest→newest,
    /// safe to call from any thread (minor tearing is acceptable for display).
    func snapshot() -> (l: [Float], r: [Float]) {
        let h = head
        var l = [Float](repeating: 0, count: Self.capacity)
        var r = [Float](repeating: 0, count: Self.capacity)
        for i in 0 ..< Self.capacity {
            let src = (h + i) % Self.capacity
            l[i] = peaksL[src]
            r[i] = peaksR[src]
        }
        return (l, r)
    }
}

// MARK: - AudioEngineManager

@MainActor
class AudioEngineManager: NSObject {
    let engine = AVAudioEngine()
    var sourceNode: AVAudioSourceNode?

    func start() {
        do {
            try engine.start()
        } catch {
            print("[AudioEngineManager] Failed to start: \(error)")
        }
    }

    func stop() {
        engine.stop()
    }
}

// MARK: - PlayerManager

@MainActor
class PlayerManager: ObservableObject {
    @Published var state          = PlayerState()
    @Published var parameters     = ParameterValues()
    @Published var isModelLoaded  = false
    @Published var error: Error?

    private let handle: EngineHandle?
    private let audioEngine  = AudioEngineManager()
    private let levels       = LevelTracker()
    let waveform             = WaveformBuffer()   // read by WaveformView via @StateObject
    private var metricsTimer: DispatchSourceTimer?

    init() {
        handle = EngineHandle()
        if handle == nil {
            print("[PlayerManager] WARNING: magentart_create() returned nil — engine unavailable.")
        }
    }

    deinit {
        metricsTimer?.cancel()
        // EngineHandle.deinit stops and destroys the runner
    }

    // MARK: - Audio engine setup

    /// Wire up AVAudioSourceNode to pull audio from the C++ engine.
    /// Call once on app launch (or after loading the first model).
    func setupAudioEngine() {
        let format = AVAudioFormat(standardFormatWithSampleRate: 48000, channels: 2)!

        // Capture by value so the render block owns its references — no retain
        // cycles and no @MainActor isolation issues on the audio thread.
        let engineRef = handle?.ref
        let levels    = self.levels
        let waveform  = self.waveform

        let source = AVAudioSourceNode(format: format) { _, _, frameCount, outputData in
            let buffers = UnsafeMutableAudioBufferListPointer(outputData)
            guard buffers.count >= 2,
                  let pL = buffers[0].mData?.assumingMemoryBound(to: Float.self),
                  let pR = buffers[1].mData?.assumingMemoryBound(to: Float.self) else {
                return noErr
            }
            let n = Int(frameCount)

            if let ref = engineRef {
                // Pull stereo samples from the lock-free ring buffer
                magentart_read_audio_stereo(ref, pL, pR, frameCount)
            } else {
                memset(pL, 0, n * MemoryLayout<Float>.size)
                memset(pR, 0, n * MemoryLayout<Float>.size)
            }

            // RMS for level meters — written lock-free, read by metrics timer
            var sumL: Float = 0, sumR: Float = 0
            for i in 0 ..< n {
                sumL += pL[i] * pL[i]
                sumR += pR[i] * pR[i]
            }
            levels.left  = n > 0 ? sqrtf(sumL / Float(n)) : 0
            levels.right = n > 0 ? sqrtf(sumR / Float(n)) : 0

            // Waveform buffer — push one peak per chunk of ~64 samples.
            // samplesPerPush peaks per render call gives ~744 display samples/sec
            // at 512-frame buffers; 512-slot capacity = ~0.7 s of rolling history.
            let pushCount = WaveformBuffer.samplesPerPush
            let chunkSize = max(1, n / pushCount)
            for c in 0 ..< pushCount {
                let start = c * chunkSize
                let end   = min(start + chunkSize, n)
                var pkL: Float = 0, pkR: Float = 0
                for i in start ..< end {
                    pkL = max(pkL, abs(pL[i]))
                    pkR = max(pkR, abs(pR[i]))
                }
                waveform.push(l: pkL, r: pkR)
            }

            return noErr
        }

        audioEngine.sourceNode = source
        audioEngine.engine.attach(source)
        audioEngine.engine.connect(source, to: audioEngine.engine.mainMixerNode, format: format)
        audioEngine.start()
    }

    // MARK: - Load model

    /// Walk up from the model file looking for a sibling `resources/` directory.
    /// Falls back to the standard `mrt models init` download location.
    private func resolveResourcesDir(for modelURL: URL) -> String {
        let fm = FileManager.default
        var dir = modelURL.deletingLastPathComponent()
        for _ in 0 ..< 5 {
            let candidate = dir.appendingPathComponent("resources")
            if fm.fileExists(atPath: candidate.path) { return candidate.path }
            let parent = dir.deletingLastPathComponent()
            if parent == dir { break }   // reached filesystem root
            dir = parent
        }
        // Standard path written by `mrt models init`
        return fm.homeDirectoryForCurrentUser
            .appendingPathComponent("Documents/Magenta/magenta-rt-v2/resources")
            .path
    }

    func loadModel(at path: String) {
        guard let ref = handle?.ref else {
            error = PlayerError.engineCreationFailed
            return
        }

        // CRITICAL: stop the inference thread before reloading model weights.
        // load_model() does NOT pause a running inference loop for you — if
        // called while state.isPlaying, the inference thread (on its own OS
        // thread) is actively reading model weight tensors via mlx::core ops
        // at the exact moment load_model() reassigns them, producing a
        // null-pointer SIGSEGV deep inside mlx::core::scheduler. Confirmed via
        // three crash reports, all identical signature. See
        // docs/mrt2-integration.md "Never Load a Model While Playing".
        if state.isPlaying {
            stop()
        }

        let url          = URL(fileURLWithPath: path)
        let name         = url.lastPathComponent
        let resourcesDir = resolveResourcesDir(for: url)

        state.modelName = "Loading \(name)…"

        Task.detached(priority: .userInitiated) { [weak self] in
            // init_assets loads SpectroStream (token→audio) and MusicCoCa (text encoder).
            // Without this, generate_frame produces silence.
            let assetsOk = magentart_init_assets(ref, resourcesDir)
            guard assetsOk else {
                await MainActor.run { [weak self] in
                    self?.state.modelName = "Not Loaded"
                    self?.error = PlayerError.assetsNotFound(resourcesDir: resourcesDir)
                }
                return
            }

            // load_model compiles the MLX transformer graph — blocks for several seconds
            let modelOk  = magentart_load_model(ref, path)
            let metrics  = magentart_get_metrics(ref)

            await MainActor.run { [weak self] in
                guard let self else { return }
                if modelOk {
                    self.state.modelName = name
                    self.state.metrics   = EngineMetrics(from: metrics)
                    self.isModelLoaded   = true
                    self.applyParameters()

                    // load_model() auto-starts the inference thread on success
                    // (realtime_runner.cpp:72 — undocumented in the header).
                    // bypass_ defaults to false, so audio would flow
                    // immediately while our own state.isPlaying stays false
                    // (we never called play()) — sound plays, button still
                    // says "Play". Stop explicitly so engine state matches UI
                    // state; the user's next Play() does a full fresh start
                    // with normal priming, as always.
                    self.stop()

                    // Persist path for auto-restore on next launch
                    UserDefaults.standard.set(path, forKey: "LastLoadedModelPath")
                } else {
                    self.state.modelName = "Not Loaded"
                    self.isModelLoaded   = false
                    self.error = PlayerError.modelLoadFailed(path: path)
                    UserDefaults.standard.removeObject(forKey: "LastLoadedModelPath")
                }
            }
        }
    }

    /// Restore the last-loaded model path if it exists and is still valid on disk.
    func restoreLastModel() {
        guard let path = UserDefaults.standard.string(forKey: "LastLoadedModelPath"),
              FileManager.default.fileExists(atPath: path) else { return }
        loadModel(at: path)
    }

    // MARK: - Playback control

    /// Start fresh: clear context, prime ring buffer, begin generating.
    func play() {
        guard let ref = handle?.ref else { error = PlayerError.engineCreationFailed; return }
        guard magentart_is_loaded(ref)  else { error = PlayerError.notLoaded; return }

        state.isPlaying = true
        state.isPaused  = false
        state.metrics   = nil             // clear stale metrics from prior session

        magentart_reset_dropped_frames(ref)   // dropped_frames counts from this play only
        magentart_set_bypass(ref, true)       // silent during prime
        magentart_trigger_reset(ref)
        magentart_start(ref)
        startMetricsTimer()

        Task {
            try? await Task.sleep(nanoseconds: 80_000_000)   // 2 × 40 ms frames
            await MainActor.run { [weak self] in
                guard let self, let ref = self.handle?.ref,
                      self.state.isPlaying, !self.state.isPaused else { return }
                magentart_set_bypass(ref, false)
            }
        }
    }

    /// Pause: bypass output, keep inference thread running, context intact.
    /// Resume is seamless — no reset, no priming delay.
    func pause() {
        guard let ref = handle?.ref, state.isPlaying, !state.isPaused else { return }
        state.isPaused = true
        magentart_set_bypass(ref, true)
        // Leave metrics timer running so the Performance panel stays live.
    }

    /// Resume from pause: unmute bypass, audio flows immediately.
    func resume() {
        guard let ref = handle?.ref, state.isPlaying, state.isPaused else { return }
        state.isPaused = false
        magentart_set_bypass(ref, false)
    }

    /// Stop: halt inference thread, clear bypass, reset state.
    func stop() {
        guard let ref = handle?.ref else { return }
        magentart_stop(ref)
        magentart_set_bypass(ref, true)
        state.isPlaying = false
        state.isPaused  = false
        stopMetricsTimer()
    }

    /// Space-bar / menu-bar smart toggle:
    ///   stopped  → play   (fresh start with reset)
    ///   playing  → pause  (silent, context preserved)
    ///   paused   → resume (seamless, no reset)
    func smartToggle() {
        if !state.isPlaying {
            play()
        } else if state.isPaused {
            resume()
        } else {
            pause()
        }
    }

    // MARK: - Parameter forwarding

    /// Push current ParameterValues to the engine. Call after load and after
    /// any parameter change in the UI.
    func applyParameters() {
        guard let ref = handle?.ref else { return }
        magentart_set_temperature(ref, parameters.temperature)
        magentart_set_top_k(ref, Int32(parameters.topk))
        magentart_set_volume_db(ref, parameters.volume)
        magentart_set_mute(ref, parameters.mute)
        magentart_set_midi_gate_enabled(ref, parameters.midigate)
        // Buffer: 4096 samples = ~85 ms = 2 inference frames of headroom.
        // Default is 2048 (~42 ms), which leaves only 2.7 ms above one frame —
        // insufficient to absorb Metal GPU scheduling jitter, causing underruns.
        magentart_set_buffer_size(ref, UInt32(parameters.buffersize))
        // Text prompt: the primary creative control — steers MusicCoCa style embedding.
        magentart_set_text_prompt(ref, parameters.textPrompt)
        // CFG weights: how strongly each conditioning signal overrides the audio context.
        magentart_set_cfg_musiccoca(ref, parameters.cfgmusiccoca)
        magentart_set_cfg_notes(ref, parameters.cfgnotes)
        magentart_set_cfg_drums(ref, parameters.cfgdrums)
    }

    /// Human-readable model description derived from the loaded filename.
    var modelDescription: String {
        let name = state.modelName
        if name == "Not Loaded" || name.hasPrefix("Loading") { return name }
        if name.contains("small") { return "\(name)  ·  230 M params" }
        if name.contains("base")  { return "\(name)  ·  2.4 B params" }
        return name
    }

    /// App version from Info.plist, or "dev" if not found.
    var appVersion: String {
        Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "dev"
    }

    /// Clear the audio context window and re-anchor to the current text prompt.
    /// Safe to call mid-playback — the transition is click-free (one-frame fade).
    /// Most effective when combined with a prompt change.
    func resetContext() {
        guard let ref = handle?.ref else { return }
        magentart_trigger_reset(ref)
    }

    func setVolume(_ db: Float) {
        parameters.volume = db
        guard let ref = handle?.ref else { return }
        magentart_set_volume_db(ref, db)
    }

    func setMute(_ muted: Bool) {
        parameters.mute = muted
        guard let ref = handle?.ref else { return }
        magentart_set_mute(ref, muted)
    }

    /// Update the text style prompt live — safe to call on every keystroke.
    /// Does not go through the full applyParameters() to avoid redundant calls.
    func setTextPrompt(_ text: String) {
        parameters.textPrompt = text
        guard let ref = handle?.ref else { return }
        magentart_set_text_prompt(ref, text)
    }

    // MARK: - Metrics polling

    private func startMetricsTimer() {
        metricsTimer?.cancel()
        let timer = DispatchSource.makeTimerSource(queue: .global(qos: .utility))
        timer.schedule(deadline: .now(), repeating: .milliseconds(100))

        let engineRef = handle?.ref
        let levels    = self.levels

        timer.setEventHandler { [weak self] in
            guard let ref = engineRef else { return }
            let m = magentart_get_metrics(ref)
            let l = levels.left
            let r = levels.right
            Task { @MainActor [weak self] in
                guard let self, self.state.isPlaying else { return }
                self.state.audioLevels = (l, r)
                self.state.metrics     = EngineMetrics(from: m)
            }
        }

        timer.resume()
        metricsTimer = timer
    }

    private func stopMetricsTimer() {
        metricsTimer?.cancel()
        metricsTimer = nil
        state.audioLevels = (0.0, 0.0)
    }
}

// MARK: - MIDIManager

class MIDIManager {
    private let midiClient: MIDIClientRef
    private let inputPort:  MIDIPortRef

    init?(onReceived: @escaping (MIDIEvent) -> Void) {
        var client: MIDIClientRef = 0
        let clientStatus = MIDIClientCreateWithBlock(
            "MagentaMIDIClient" as CFString, &client) { _ in }
        guard clientStatus == noErr else { return nil }
        midiClient = client

        var port: MIDIPortRef = 0
        let portStatus = MIDIInputPortCreateWithBlock(
            client, "MagentaInputPort" as CFString, &port) { _, _ in }
        guard portStatus == noErr else {
            MIDIClientDispose(client)
            return nil
        }
        inputPort = port
    }

    deinit {
        MIDIPortDispose(inputPort)
        MIDIClientDispose(midiClient)
    }
}
