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
private final class LevelTracker: @unchecked Sendable {
    var left:  Float = 0
    var right: Float = 0
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
    @Published var state      = PlayerState()
    @Published var parameters = ParameterValues()
    @Published var error: Error?

    private let handle: EngineHandle?
    private let audioEngine  = AudioEngineManager()
    private let levels       = LevelTracker()
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
                    self.applyParameters()
                } else {
                    self.state.modelName = "Not Loaded"
                    self.error = PlayerError.modelLoadFailed(path: path)
                }
            }
        }
    }

    // MARK: - Play / Stop

    func togglePlay() {
        guard let ref = handle?.ref else {
            error = PlayerError.engineCreationFailed
            return
        }
        guard magentart_is_loaded(ref) else {
            error = PlayerError.notLoaded
            return
        }

        state.isPlaying.toggle()

        if state.isPlaying {
            magentart_set_bypass(ref, true)   // stay silent while buffer primes
            magentart_trigger_reset(ref)
            magentart_start(ref)
            startMetricsTimer()
            // Wait 2 inference frame durations (2 × 40 ms = 80 ms) so the
            // ring buffer has audio before the render block starts pulling.
            // Without this priming delay the very first reads are zero-padded
            // and produce audible wobble at playback start.
            Task {
                try? await Task.sleep(nanoseconds: 80_000_000)
                await MainActor.run { [weak self] in
                    guard let self, let ref = self.handle?.ref,
                          self.state.isPlaying else { return }
                    magentart_set_bypass(ref, false)
                }
            }
        } else {
            magentart_stop(ref)
            magentart_set_bypass(ref, true)
            stopMetricsTimer()
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
