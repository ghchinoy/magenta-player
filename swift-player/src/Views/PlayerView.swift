import SwiftUI

struct PlayerView: View {
    @StateObject private var playerManager = PlayerManager()

    var body: some View {
        VStack(spacing: 20) {

            // Header
            VStack(spacing: 4) {
                Text("Magenta RealTime")
                    .font(.largeTitle)
                    .fontWeight(.bold)
                Text(playerManager.state.modelName)
                    .font(.subheadline)
                    .foregroundColor(.secondary)
            }
            .padding(.top)

            // Main controls
            HStack(spacing: 16) {
                PlayPauseButton(isPlaying: playerManager.state.isPlaying) {
                    playerManager.togglePlay()
                }
                LoadModelButton {
                    openModelPicker()
                }
            }

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
            playerManager.togglePlay()
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

struct PlayPauseButton: View {
    let isPlaying: Bool
    let action: () -> Void
    @State private var isHovering = false

    var body: some View {
        Button(action: action) {
            HStack(spacing: 8) {
                Image(systemName: isPlaying ? "stop.circle.fill" : "play.circle.fill")
                    .font(.system(size: 28))
                Text(isPlaying ? "Stop" : "Play")
                    .font(.title3).fontWeight(.semibold)
            }
            .padding(.horizontal, 20)
            .padding(.vertical, 10)
            .background(isPlaying ? Color.red : Color.green)
            .foregroundColor(.white)
            .cornerRadius(10)
            .shadow(radius: isHovering ? 6 : 3)
            .scaleEffect(isHovering ? 1.03 : 1.0)
            .animation(.easeOut(duration: 0.15), value: isHovering)
        }
        .buttonStyle(.plain)
        .onHover { hovering in
            isHovering = hovering
            if hovering { NSCursor.pointingHand.push() } else { NSCursor.pop() }
        }
        .help(isPlaying ? "Stop generation (Space)" : "Start generation (Space)")
    }
}

struct LoadModelButton: View {
    let action: () -> Void
    @State private var isHovering = false

    var body: some View {
        Button(action: action) {
            HStack(spacing: 8) {
                Image(systemName: "folder.badge.plus")
                    .font(.system(size: 20))
                Text("Load Model")
                    .font(.title3).fontWeight(.semibold)
            }
            .padding(.horizontal, 20)
            .padding(.vertical, 10)
            .background(Color.accentColor)
            .foregroundColor(.white)
            .cornerRadius(10)
            .shadow(radius: isHovering ? 6 : 3)
            .scaleEffect(isHovering ? 1.03 : 1.0)
            .animation(.easeOut(duration: 0.15), value: isHovering)
        }
        .buttonStyle(.plain)
        .onHover { hovering in
            isHovering = hovering
            if hovering { NSCursor.pointingHand.push() } else { NSCursor.pop() }
        }
        .help("Load a Magenta RealTime model (⌘O)")
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

struct PlayerView_Previews: PreviewProvider {
    static var previews: some View {
        PlayerView()
    }
}
