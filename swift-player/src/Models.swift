import Foundation
import AVFoundation
import CoreMIDI

struct EngineMetrics {
    let transformerMs: Float
    let totalMs: Float
    let bufferAvailable: Int
    let bufferCapacity: Int
    let transportFlags: Int
    let droppedFrames: UInt64

    /// Initialise from the C bridge metrics snapshot.
    init(from m: MagentaMetrics) {
        transformerMs   = m.transformer_ms
        totalMs         = m.total_ms
        bufferAvailable = Int(m.buffer_available)
        bufferCapacity  = Int(m.buffer_capacity)
        transportFlags  = Int(m.transport_flags)
        droppedFrames   = m.dropped_frames
    }
}

extension EngineMetrics {
    /// Memberwise init kept for tests and previews.
    init(transformerMs: Float, totalMs: Float,
         bufferAvailable: Int, bufferCapacity: Int,
         transportFlags: Int, droppedFrames: UInt64) {
        self.transformerMs   = transformerMs
        self.totalMs         = totalMs
        self.bufferAvailable = bufferAvailable
        self.bufferCapacity  = bufferCapacity
        self.transportFlags  = transportFlags
        self.droppedFrames   = droppedFrames
    }
}

struct PlayerState {
    var isPlaying  = false  // inference thread is running (includes paused)
    var isPaused   = false  // bypass is on — silent but context preserved
    var modelName  = "Not Loaded"
    var audioLevels: (left: Float, right: Float) = (0.0, 0.0)
    var metrics: EngineMetrics?
    var midiSources: [MIDIEndpointRef] = []

    /// True only when inference is running AND audio is flowing (not bypassed).
    var isGenerating: Bool { isPlaying && !isPaused }
}

struct ParameterValues {
    /// Free-text style prompt forwarded to MusicCoCa for embedding.
    /// This is the primary creative control — steers the musical style of
    /// generation. Empty string = unconditioned (random/average) output.
    var textPrompt: String = ""
    var temperature: Float = 1.3
    var topk: Int = 40
    var cfgmusiccoca: Float = 3.0
    var cfgnotes: Float = 1.0
    var cfgdrums: Float = 1.0
    var volume: Float = 0.0   // dB — 0.0 = unity gain, range -40 to +6
    var mute: Bool = false
    var unmaskwidth: Int = 4
    // 8192 = max ring buffer capacity (~170 ms, 4 inference frames).
    // mrt2_base inference can take 50–80 ms on M1 Pro; this gives enough
    // headroom to absorb variance without underruns on supported hardware.
    var buffersize: Int = 8192
    var latencycomp: Bool = false
    var weight0: Float = 0.0
    var weight1: Float = 0.0
    var weight2: Float = 0.0
    var weight3: Float = 0.0
    var weight4: Float = 0.0
    var weight5: Float = 0.0
    var drumless: Bool = false
    var midigate: Bool = false
    var onsetmode: Bool = false
}

struct MIDIEvent {
    let note: UInt8
    let velocity: UInt8
    let isOn: Bool
}
