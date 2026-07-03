import SwiftUI

struct PlayerView: View {
    @StateObject private var playerManager = PlayerManager()
    @StateObject private var downloader    = ModelDownloader()
    @State private var showingAbout    = false
    @State private var showingDownload = false

    var body: some View {
        VStack(spacing: 20) {

            // Header
            VStack(spacing: 4) {
                Text("Magenta RealTime")
                    .font(.largeTitle)
                    .fontWeight(.bold)
                HStack(spacing: 6) {
                    Text(playerManager.modelDescription)
                        .font(.subheadline)
                        .foregroundColor(.secondary)
                    CompactIconButton(
                        systemImage: "folder.badge.plus",
                        help: "Load a Magenta RealTime model (⌘O)"
                    ) {
                        openModelPicker()
                    }
                    CompactIconButton(
                        systemImage: "icloud.and.arrow.down",
                        help: "Download a model from HuggingFace"
                    ) {
                        showingDownload = true
                    }
                }
                .animation(.easeInOut(duration: 0.2), value: playerManager.modelDescription)
            }
            .padding(.top)

            // Style prompt — primary creative control
            PromptField(text: $playerManager.parameters.textPrompt) {
                playerManager.setTextPrompt(playerManager.parameters.textPrompt)
            }
            .onChange(of: playerManager.parameters.textPrompt) { _, newValue in
                playerManager.setTextPrompt(newValue)
            }

            // Prompt Strength — cfg_musiccoca (how strongly the prompt steers generation)
            PromptStrengthSlider(
                value: $playerManager.parameters.cfgmusiccoca,
                onChange: { playerManager.applyParameters() }
            )

            // Advanced guidance — cfg_notes / cfg_drums (secondary CFG weights)
            AdvancedGuidanceSection(
                cfgNotes: $playerManager.parameters.cfgnotes,
                cfgDrums: $playerManager.parameters.cfgdrums,
                onChange: { playerManager.applyParameters() }
            )

            // Main controls — layout changes with playback state
            HStack(spacing: 12) {
                // Primary action: Play / Pause / Resume
                MainPlaybackButton(state: playerManager.state,
                                   isEnabled: playerManager.isModelLoaded) {
                    playerManager.smartToggle()
                }

                // Stop — visible when inference thread is running (playing or paused)
                if playerManager.state.isPlaying {
                    StopButton { playerManager.stop() }
                        .transition(.scale.combined(with: .opacity))
                }

                // Reset — visible only while actively generating (not while paused)
                if playerManager.state.isGenerating {
                    ResetContextButton { playerManager.resetContext() }
                        .transition(.scale.combined(with: .opacity))
                }
            }
            .animation(.easeOut(duration: 0.15), value: playerManager.state.isPlaying)
            .animation(.easeOut(duration: 0.15), value: playerManager.state.isPaused)

            // Hint shown until first model is loaded
            if !playerManager.isModelLoaded {
                Text("Load a model to begin")
                    .font(.caption)
                    .foregroundColor(.secondary)
                    .transition(.opacity)
            }

            // Volume and mute
            VolumeRow(
                volume: $playerManager.parameters.volume,
                muted: $playerManager.parameters.mute,
                onVolumeChange: { playerManager.setVolume(playerManager.parameters.volume) },
                onMuteToggle:   { playerManager.setMute(playerManager.parameters.mute) }
            )

            // Rolling waveform visualiser
            WaveformView(waveform: playerManager.waveform,
                         isPlaying: playerManager.state.isGenerating)
                .frame(height: 72)
                .padding(.horizontal)

            Divider()

            // Audio levels
            VStack(spacing: 8) {
                Text("Audio Levels")
                    .font(.headline)
                HStack(spacing: 40) {
                    ChannelLevel(label: "L",
                                 value: playerManager.state.audioLevels.left,
                                 color: .blue)
                    ChannelLevel(label: "R",
                                 value: playerManager.state.audioLevels.right,
                                 color: .green)
                }
            }
            .padding(.horizontal)

            // Performance metrics (visible once a model is loaded)
            if let metrics = playerManager.state.metrics {
                Divider()
                VStack(spacing: 8) {
                    Text("Performance")
                        .font(.headline)
                    HStack(spacing: 40) {
                        MetricCell(label: "Frame Time",
                                   value: String(format: "%.1f ms", metrics.transformerMs))
                        MetricCell(label: "Buffer",
                                   value: "\(metrics.bufferAvailable) / \(metrics.bufferCapacity)")
                        MetricCell(label: "Dropped",
                                   value: "\(metrics.droppedFrames)")
                    }
                }
                .padding(.horizontal)
            }

            Spacer()

            // MIDI sources (visible when connected)
            if !playerManager.state.midiSources.isEmpty {
                Divider()
                VStack(spacing: 4) {
                    Text("MIDI Sources").font(.headline)
                    ForEach(playerManager.state.midiSources, id: \.self) { _ in
                        Text("Connected MIDI Source")
                            .font(.caption)
                            .foregroundColor(.secondary)
                    }
                }
                .padding(.horizontal)
            }
        }
        .padding(.bottom)
        // Start AVAudioEngine on first appearance — must happen before playback
        .onAppear {
            playerManager.setupAudioEngine()
        }
        // Wire up error reporting
        .errorAlert($playerManager.error)
        // Receive menu-bar commands (Cmd+O / Space)
        .onReceive(NotificationCenter.default.publisher(for: .magentaLoadModel)) { _ in
            openModelPicker()
        }
        .onReceive(NotificationCenter.default.publisher(for: .magentaTogglePlay)) { _ in
            playerManager.smartToggle()
        }
        .onReceive(NotificationCenter.default.publisher(for: .magentaStop)) { _ in
            playerManager.stop()
        }
        .onReceive(NotificationCenter.default.publisher(for: .magentaResetContext)) { _ in
            playerManager.resetContext()
        }
        .onReceive(NotificationCenter.default.publisher(for: .magentaShowAbout)) { _ in
            showingAbout = true
        }
        .sheet(isPresented: $showingAbout) {
            AboutView(
                version: playerManager.appVersion,
                modelDescription: playerManager.modelDescription
            )
        }
        .sheet(isPresented: $showingDownload) {
            DownloadModelSheet(downloader: downloader)
        }
        .onAppear {
            // Wire once: when a download finishes, auto-load it and dismiss.
            downloader.onComplete = { [weak playerManager] path in
                playerManager?.loadModel(at: path)
                showingDownload = false
            }
        }
    }

    private func openModelPicker() {
        let panel = NSOpenPanel()
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = true
        panel.canChooseFiles = true
        panel.message = "Select a Magenta RealTime model folder or .mlxfn file"
        if panel.runModal() == .OK, let url = panel.url {
            playerManager.loadModel(at: url.path)
        }
    }
}

// MARK: - Sub-views

struct MainPlaybackButton: View {
    let state: PlayerState
    var isEnabled: Bool = true
    let action: () -> Void
    @State private var isHovering = false

    private var icon: String {
        if state.isPaused  { return "play.circle.fill" }
        if state.isPlaying { return "pause.circle.fill" }
        return "play.circle.fill"
    }
    private var label: String {
        if state.isPaused  { return "Resume" }
        if state.isPlaying { return "Pause"  }
        return "Play"
    }
    private var bg: Color {
        guard isEnabled else { return Color.secondary.opacity(0.2) }
        if state.isPaused  { return .green }
        if state.isPlaying { return .orange }
        return .green
    }
    private var helpText: String {
        guard isEnabled else { return "Load a model first (⌘O)" }
        if state.isPaused  { return "Resume generation (Space)" }
        if state.isPlaying { return "Pause — keeps context, silent (Space)" }
        return "Start generation (Space)"
    }

    var body: some View {
        Button(action: action) {
            HStack(spacing: 8) {
                Image(systemName: icon).font(.system(size: 28))
                Text(label).font(.title3).fontWeight(.semibold)
            }
            .padding(.horizontal, 20)
            .padding(.vertical, 10)
            .background(bg)
            .foregroundColor(isEnabled ? .white : .secondary)
            .cornerRadius(10)
            .shadow(radius: isEnabled && isHovering ? 6 : isEnabled ? 3 : 0)
            .scaleEffect(isEnabled && isHovering ? 1.03 : 1.0)
            .animation(.easeOut(duration: 0.15), value: isHovering)
        }
        .buttonStyle(.plain)
        .disabled(!isEnabled)
        .onHover { hovering in
            isHovering = hovering
            guard isEnabled else { return }
            if hovering { NSCursor.pointingHand.push() } else { NSCursor.pop() }
        }
        .help(helpText)
    }
}

struct StopButton: View {
    let action: () -> Void
    @State private var isHovering = false

    var body: some View {
        Button(action: action) {
            HStack(spacing: 6) {
                Image(systemName: "stop.circle.fill").font(.system(size: 22))
                Text("Stop").font(.body).fontWeight(.semibold)
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 10)
            .background(Color.red)
            .foregroundColor(.white)
            .cornerRadius(10)
            .shadow(radius: isHovering ? 5 : 2)
            .scaleEffect(isHovering ? 1.03 : 1.0)
            .animation(.easeOut(duration: 0.15), value: isHovering)
        }
        .buttonStyle(.plain)
        .onHover { hovering in
            isHovering = hovering
            if hovering { NSCursor.pointingHand.push() } else { NSCursor.pop() }
        }
        .help("Stop generation and clear context (⌘.)")
    }
}

/// Small icon-only button, sized to sit inline with the model description
/// text under the header. Used for Load Model and Download Model — both
/// moved out of the main playback controls row so that row stays focused
/// on transport (Play/Pause/Resume/Stop/Reset).
struct CompactIconButton: View {
    let systemImage: String
    let help: String
    let action: () -> Void
    @State private var isHovering = false

    var body: some View {
        Button(action: action) {
            Image(systemName: systemImage)
                .font(.system(size: 12, weight: .medium))
                .foregroundColor(isHovering ? .accentColor : .secondary)
                .frame(width: 22, height: 22)
                .background(
                    Circle().fill(isHovering ? Color.secondary.opacity(0.15) : Color.clear)
                )
        }
        .buttonStyle(.plain)
        .onHover { hovering in
            isHovering = hovering
            if hovering { NSCursor.pointingHand.push() } else { NSCursor.pop() }
        }
        .help(help)
    }
}

// MARK: - Download Model sheet

private struct DownloadableModel: Identifiable {
    let id: String          // "mrt2_small" / "mrt2_base"
    let params: String
    let hardware: String
    let modelBytes: Int64   // combined .mlxfn + _state.safetensors
}

private let downloadableModels: [DownloadableModel] = [
    .init(id: "mrt2_small", params: "230 M params",
          hardware: "Any Apple Silicon", modelBytes: 464_331_548),
    .init(id: "mrt2_base", params: "2.4 B params",
          hardware: "M1 Pro / Max and above", modelBytes: 2_788_354_715),
]

private let resourcesBytes: Int64 = 1_375_741_343   // musiccoca + spectrostream, one-time

private let byteFormatter: ByteCountFormatter = {
    let f = ByteCountFormatter()
    f.countStyle = .file
    return f
}()

struct DownloadModelSheet: View {
    @ObservedObject var downloader: ModelDownloader
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(spacing: 0) {
            Text("Download a Model")
                .font(.title2).fontWeight(.bold)
                .padding(.top, 24)
            Text("From HuggingFace — google/magenta-realtime-2")
                .font(.caption)
                .foregroundColor(.secondary)
                .padding(.bottom, 20)

            if downloader.isDownloading {
                downloadingView
            } else {
                modelList
            }

            Divider()
            HStack {
                if downloader.isDownloading {
                    Button("Cancel") { downloader.cancel() }
                } else {
                    Button("Close") { dismiss() }
                }
                Spacer()
            }
            .padding()
        }
        .frame(width: 420)
    }

    private var modelList: some View {
        VStack(spacing: 12) {
            if !downloader.resourcesExist() {
                Label(
                    "First download also fetches \(byteFormatter.string(fromByteCount: resourcesBytes)) of shared codec resources (one-time).",
                    systemImage: "info.circle"
                )
                .font(.caption2)
                .foregroundColor(.secondary)
                .padding(.horizontal)
            }

            ForEach(downloadableModels) { model in
                ModelDownloadRow(
                    model: model,
                    alreadyDownloaded: downloader.modelExists(model.id),
                    onDownload: { downloader.startDownload(model: model.id) },
                    onLoad: {
                        downloader.onComplete?(downloader.modelPath(model.id))
                    }
                )
            }
        }
        .padding(.horizontal)
        .padding(.bottom, 20)
    }

    private var downloadingView: some View {
        VStack(spacing: 12) {
            ProgressView(value: downloader.overallProgress)
                .progressViewStyle(.linear)
            HStack {
                Text(downloader.statusMessage)
                    .font(.caption)
                    .foregroundColor(.secondary)
                    .lineLimit(1)
                    .truncationMode(.middle)
                Spacer()
                Text("\(Int(downloader.overallProgress * 100))%")
                    .font(.caption.monospacedDigit())
                    .foregroundColor(.secondary)
            }
            if let err = downloader.errorMessage {
                Text(err)
                    .font(.caption)
                    .foregroundColor(.red)
            }
        }
        .padding(.horizontal)
        .padding(.bottom, 20)
    }
}

private struct ModelDownloadRow: View {
    let model: DownloadableModel
    let alreadyDownloaded: Bool
    let onDownload: () -> Void
    let onLoad: () -> Void

    var body: some View {
        HStack {
            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 6) {
                    Text(model.id).font(.body).fontWeight(.semibold)
                    if alreadyDownloaded {
                        Image(systemName: "checkmark.circle.fill")
                            .foregroundColor(.green)
                            .font(.caption)
                    }
                }
                Text("\(model.params) · \(model.hardware)")
                    .font(.caption2)
                    .foregroundColor(.secondary)
                Text(byteFormatter.string(fromByteCount: model.modelBytes))
                    .font(.caption2)
                    .foregroundColor(.secondary)
            }
            Spacer()
            Button(alreadyDownloaded ? "Load" : "Download") {
                alreadyDownloaded ? onLoad() : onDownload()
            }
            .buttonStyle(.borderedProminent)
            .tint(alreadyDownloaded ? .green : .indigo)
        }
        .padding(.vertical, 8)
        .padding(.horizontal, 12)
        .background(Color.secondary.opacity(0.08))
        .cornerRadius(8)
    }
}

// MARK: - Volume row

struct VolumeRow: View {
    @Binding var volume: Float   // dB, range -40...+6
    @Binding var muted: Bool
    let onVolumeChange: () -> Void
    let onMuteToggle: () -> Void

    private let range: ClosedRange<Float> = -40.0...6.0

    var body: some View {
        HStack(spacing: 12) {
            // Mute toggle
            Button {
                muted.toggle()
                onMuteToggle()
            } label: {
                Image(systemName: muted ? "speaker.slash.fill" : "speaker.wave.2.fill")
                    .font(.system(size: 16))
                    .foregroundColor(muted ? .red : .primary)
                    .frame(width: 22)
            }
            .buttonStyle(.plain)
            .help(muted ? "Unmute" : "Mute")

            // Volume slider
            Slider(value: Binding(
                get: { Double(volume) },
                set: { volume = Float($0); onVolumeChange() }
            ), in: Double(range.lowerBound)...Double(range.upperBound), step: 0.5)
            .tint(muted ? .secondary : .accentColor)
            .disabled(muted)

            // Value label
            Text(volume >= -39.5
                 ? String(format: "%+.0f dB", volume)
                 : "-∞")
                .font(.caption.monospacedDigit())
                .foregroundColor(.secondary)
                .frame(width: 42, alignment: .trailing)
        }
        .padding(.horizontal)
        .opacity(muted ? 0.55 : 1.0)
        .animation(.easeOut(duration: 0.15), value: muted)
    }
}

// MARK: - Waveform visualiser

struct WaveformView: View {
    let waveform: WaveformBuffer
    let isPlaying: Bool

    var body: some View {
        TimelineView(.animation(minimumInterval: 1.0 / 30)) { context in
            Canvas { ctx, size in
                // Capture context.date to establish an explicit dependency on
                // the timeline tick. Without this, SwiftUI sees WaveformBuffer
                // (a reference type) as unchanged between frames and caches the
                // Canvas output — the waveform appears frozen even though the
                // ring buffer is updating at 30+ fps on the audio thread.
                _ = context.date

                let (snapL, snapR) = waveform.snapshot()
                let count = snapL.count
                guard count > 1 else { return }

                let w = size.width
                let h = size.height
                let midY = h * 0.5
                let amp  = midY * 0.9   // max excursion from centre

                // Background
                ctx.fill(
                    Path(CGRect(origin: .zero, size: size)),
                    with: .color(Color(nsColor: .controlBackgroundColor).opacity(0.6))
                )

                // Draw one channel as a filled polyline
                func polyline(peaks: [Float], color: Color) {
                    var path = Path()
                    for i in 0 ..< count {
                        let x = CGFloat(i) / CGFloat(count - 1) * w
                        let y = midY - CGFloat(peaks[i]) * amp
                        if i == 0 { path.move(to: CGPoint(x: x, y: y)) }
                        else       { path.addLine(to: CGPoint(x: x, y: y)) }
                    }
                    // Mirror to fill the waveform shape
                    for i in stride(from: count - 1, through: 0, by: -1) {
                        let x = CGFloat(i) / CGFloat(count - 1) * w
                        let y = midY + CGFloat(peaks[i]) * amp
                        path.addLine(to: CGPoint(x: x, y: y))
                    }
                    path.closeSubpath()
                    ctx.fill(path, with: .color(color.opacity(0.25)))
                    // Outline
                    var line = Path()
                    for i in 0 ..< count {
                        let x = CGFloat(i) / CGFloat(count - 1) * w
                        let y = midY - CGFloat(peaks[i]) * amp
                        if i == 0 { line.move(to: CGPoint(x: x, y: y)) }
                        else       { line.addLine(to: CGPoint(x: x, y: y)) }
                    }
                    ctx.stroke(line, with: .color(color.opacity(0.8)), lineWidth: 1)
                }

                polyline(peaks: snapL, color: .blue)
                polyline(peaks: snapR, color: .green)

                // Centre line
                var centre = Path()
                centre.move(to: CGPoint(x: 0, y: midY))
                centre.addLine(to: CGPoint(x: w, y: midY))
                ctx.stroke(centre,
                           with: .color(Color.secondary.opacity(0.2)),
                           lineWidth: 0.5)
            }
            .cornerRadius(8)
            .overlay(
                RoundedRectangle(cornerRadius: 8)
                    .stroke(Color.secondary.opacity(0.15), lineWidth: 1)
            )
            .opacity(isPlaying ? 1.0 : 0.35)
            .animation(.easeOut(duration: 0.3), value: isPlaying)
        }
    }
}

// MARK: - Prompt Strength slider (cfg_musiccoca)

struct PromptStrengthSlider: View {
    @Binding var value: Float
    let onChange: () -> Void

    private let range: ClosedRange<Float> = 1.0...8.0

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                Label("Prompt Strength", systemImage: "dial.medium")
                    .font(.caption)
                    .foregroundColor(.secondary)
                Spacer()
                Text(String(format: "%.1f", value))
                    .font(.caption.monospacedDigit())
                    .foregroundColor(.secondary)
                    .frame(width: 28, alignment: .trailing)
            }

            HStack(spacing: 8) {
                Text("Free")
                    .font(.caption2)
                    .foregroundColor(.secondary)
                Slider(value: Binding(
                    get: { Double(value) },
                    set: { value = Float($0); onChange() }
                ), in: Double(range.lowerBound)...Double(range.upperBound), step: 0.1)
                .tint(.accentColor)
                Text("Locked")
                    .font(.caption2)
                    .foregroundColor(.secondary)
            }
        }
        .padding(.horizontal)
    }
}

// MARK: - Advanced guidance (cfg_notes / cfg_drums)

struct AdvancedGuidanceSection: View {
    @Binding var cfgNotes: Float
    @Binding var cfgDrums: Float
    let onChange: () -> Void
    @State private var isExpanded = false

    var body: some View {
        DisclosureGroup(isExpanded: $isExpanded) {
            VStack(spacing: 10) {
                CFGSlider(
                    label: "Note Guidance", icon: "pianokeys",
                    value: $cfgNotes, range: 0.0...4.0,
                    onChange: onChange
                )
                CFGSlider(
                    label: "Drum Guidance", icon: "metronome",
                    value: $cfgDrums, range: 0.0...4.0,
                    onChange: onChange
                )
            }
            .padding(.top, 10)
        } label: {
            Label("Advanced Guidance", systemImage: "slider.horizontal.3")
                .font(.caption)
                .foregroundColor(.secondary)
        }
        .padding(.horizontal)
    }
}

struct CFGSlider: View {
    let label: String
    let icon: String
    @Binding var value: Float
    let range: ClosedRange<Float>
    let onChange: () -> Void

    var body: some View {
        HStack(spacing: 10) {
            Label(label, systemImage: icon)
                .font(.caption2)
                .foregroundColor(.secondary)
                .frame(width: 100, alignment: .leading)
            Slider(value: Binding(
                get: { Double(value) },
                set: { value = Float($0); onChange() }
            ), in: Double(range.lowerBound)...Double(range.upperBound), step: 0.1)
            .tint(.accentColor)
            Text(String(format: "%.1f", value))
                .font(.caption2.monospacedDigit())
                .foregroundColor(.secondary)
                .frame(width: 24, alignment: .trailing)
        }
    }
}

// MARK: - Reset Context button

struct ResetContextButton: View {
    let action: () -> Void
    @State private var isHovering = false
    @State private var didFlash = false

    var body: some View {
        Button(action: {
            action()
            withAnimation(.easeOut(duration: 0.12)) { didFlash = true }
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.25) {
                withAnimation(.easeOut(duration: 0.2)) { didFlash = false }
            }
        }) {
            HStack(spacing: 6) {
                Image(systemName: "arrow.counterclockwise")
                    .font(.system(size: 16, weight: .semibold))
                Text("Reset")
                    .font(.title3).fontWeight(.semibold)
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 10)
            .background(didFlash ? Color.orange : Color.secondary.opacity(isHovering ? 0.25 : 0.15))
            .foregroundColor(didFlash ? .white : .primary)
            .cornerRadius(10)
            .shadow(radius: isHovering ? 4 : 2)
            .scaleEffect(isHovering ? 1.03 : 1.0)
            .animation(.easeOut(duration: 0.15), value: isHovering)
            .animation(.easeOut(duration: 0.12), value: didFlash)
        }
        .buttonStyle(.plain)
        .onHover { hovering in
            isHovering = hovering
            if hovering { NSCursor.pointingHand.push() } else { NSCursor.pop() }
        }
        .help("Reset audio context — re-anchors generation to the current prompt (⌘R)")
    }
}

struct PromptField: View {
    @Binding var text: String
    let onSubmit: () -> Void
    @FocusState private var isFocused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Label("Style Prompt", systemImage: "music.mic")
                .font(.caption)
                .foregroundColor(.secondary)

            HStack(spacing: 8) {
                TextField(
                    "e.g. jazz piano trio, ambient synth, heavy metal drums…",
                    text: $text
                )
                .textFieldStyle(.plain)
                .font(.body)
                .focused($isFocused)
                .onSubmit { onSubmit() }

                // Clear button — visible when there is text
                if !text.isEmpty {
                    Button {
                        text = ""
                        onSubmit()
                    } label: {
                        Image(systemName: "xmark.circle.fill")
                            .foregroundColor(.secondary)
                    }
                    .buttonStyle(.plain)
                    .help("Clear prompt")
                }
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
            .background(Color(nsColor: .textBackgroundColor))
            .cornerRadius(8)
            .overlay(
                RoundedRectangle(cornerRadius: 8)
                    .stroke(
                        isFocused
                            ? Color.accentColor.opacity(0.8)
                            : text.isEmpty
                                ? Color.secondary.opacity(0.25)
                                : Color.accentColor.opacity(0.45),
                        lineWidth: isFocused ? 1.5 : 1
                    )
            )
        }
        .padding(.horizontal)
    }
}

struct ChannelLevel: View {
    let label: String
    let value: Float   // 0.0 – 1.0 linear amplitude
    let color: Color

    private var dBFS: Float {
        value > 0 ? 20 * log10(value) : -Float.infinity
    }

    var body: some View {
        VStack(spacing: 4) {
            Text(label).font(.caption).foregroundColor(.secondary)
            Text(value > 0
                 ? String(format: "%.1f dB", dBFS)
                 : "–∞ dB")
                .font(.system(.title2, design: .monospaced))
                .fontWeight(.bold)
                .foregroundColor(color)
        }
    }
}

struct MetricCell: View {
    let label: String
    let value: String

    var body: some View {
        VStack(spacing: 4) {
            Text(label).font(.caption).foregroundColor(.secondary)
            Text(value)
                .font(.system(.body, design: .monospaced))
                .fontWeight(.semibold)
        }
    }
}

// MARK: - About sheet

struct AboutView: View {
    let version: String
    let modelDescription: String
    @Environment(\.dismiss) private var dismiss

    // Resolve the about-logo from the app bundle Resources/ or the source tree
    private var logoImage: NSImage? {
        if let img = NSImage(named: "about-logo") { return img }
        // Fallback: locate relative to this file during development
        let dev = URL(fileURLWithPath: #file)
            .deletingLastPathComponent()          // Views/
            .deletingLastPathComponent()          // src/
            .deletingLastPathComponent()          // swift-player/
            .appendingPathComponent("Resources/about-logo.png")
        return NSImage(contentsOf: dev)
    }

    var body: some View {
        VStack(spacing: 0) {
            // Logo
            Group {
                if let img = logoImage {
                    Image(nsImage: img)
                        .resizable()
                        .interpolation(.high)
                        .aspectRatio(contentMode: .fit)
                        .frame(width: 160, height: 160)
                        .cornerRadius(32)
                        .shadow(radius: 12)
                } else {
                    Image(systemName: "waveform.circle.fill")
                        .font(.system(size: 80))
                        .foregroundColor(.accentColor)
                }
            }
            .padding(.top, 32)
            .padding(.bottom, 20)

            // App name + version
            Text("Magenta Player")
                .font(.title).fontWeight(.bold)
            Text("Version \(version)")
                .font(.subheadline)
                .foregroundColor(.secondary)
                .padding(.bottom, 12)

            Divider().padding(.horizontal, 40)

            // Model info
            VStack(spacing: 4) {
                Text("Loaded Model")
                    .font(.caption)
                    .foregroundColor(.secondary)
                    .textCase(.uppercase)
                    .tracking(0.8)
                Text(modelDescription)
                    .font(.body.monospacedDigit())
            }
            .padding(.vertical, 16)

            Divider().padding(.horizontal, 40)

            // Links
            VStack(spacing: 8) {
                Link("magenta-realtime on GitHub",
                     destination: URL(string: "https://github.com/magenta/magenta-realtime")!)
                Link("Magenta RealTime 2 — Project Page",
                     destination: URL(string: "https://magenta.withgoogle.com/mrt2")!)
            }
            .font(.footnote)
            .padding(.vertical, 16)

            // Close
            Button("Close") { dismiss() }
                .keyboardShortcut(.defaultAction)
                .padding(.bottom, 24)
        }
        .frame(width: 340)
    }
}

struct PlayerView_Previews: PreviewProvider {
    static var previews: some View {
        PlayerView()
    }
}
