import Foundation

// ModelDownloader.swift
// ====================
// Downloads MRT2 model weights and shared codec resources directly from
// HuggingFace, without any Python or `mrt` CLI dependency.
//
// Uses the public HF REST API:
//   List:     GET https://huggingface.co/api/models/{repo}/tree/main/{path}
//   Download: GET https://huggingface.co/{repo}/resolve/main/{path}
//             (302 redirects to the CDN — URLSession follows automatically)
//
// Repo layout (google/magenta-realtime-2):
//   models/mrt2_small/{mrt2_small.mlxfn, mrt2_small_state.safetensors}
//   models/mrt2_base/{mrt2_base.mlxfn, mrt2_base_state.safetensors}
//   resources/musiccoca/*.tflite + spm.model      (~957 MB, shared, one-time)
//   resources/spectrostream/*.safetensors + .mlxfn (~418 MB, shared, one-time)
//
// Without the resources, init_assets() fails and the engine falls back to a
// hardcoded default prompt embedding — silent failure. See
// docs/realtime-audio.md "init_assets() Is Required Before Prompts Work".
// This downloader always includes resources on first download so users never
// hit that trap.

/// One remote file entry from the HF tree API.
struct HFFile: Decodable {
    let path: String
    let size: Int64
}

@MainActor
final class ModelDownloader: NSObject, ObservableObject {
    @Published var isDownloading = false
    @Published var currentFileName = ""
    @Published var overallProgress: Double = 0     // 0...1 across all queued files
    @Published var statusMessage = ""
    @Published var errorMessage: String?

    /// Called once, with the local path to the downloaded .mlxfn file.
    var onComplete: ((String) -> Void)?

    private let repo = "google/magenta-realtime-2"
    private var queue: [HFFile] = []
    private var completedBytes: Int64 = 0
    private var totalBytes: Int64 = 0
    private var currentModel = ""

    private lazy var session: URLSession = URLSession(
        configuration: .default, delegate: self, delegateQueue: nil
    )

    /// Guards `pendingDestination`/`pendingFileSize`, which are written on the
    /// main actor (before starting each download task) and read from the
    /// URLSession delegate queue (a background thread) in
    /// `didFinishDownloadingTo`. That callback must move the temp file
    /// *synchronously* before returning — the OS deletes the temp file the
    /// instant the delegate method returns — so this cannot be an actor hop.
    private let stateLock = NSLock()
    // `nonisolated(unsafe)`: these are read from the URLSession delegate
    // queue (a background thread) inside `didFinishDownloadingTo`, which
    // must run synchronously — see the comment above `stateLock`. Safety is
    // provided by `stateLock`, not by actor isolation.
    private nonisolated(unsafe) var pendingDestination: URL?
    private nonisolated(unsafe) var pendingFileSize: Int64 = 0

    static let magentaHome = FileManager.default.homeDirectoryForCurrentUser
        .appendingPathComponent("Documents/Magenta/magenta-rt-v2")

    // MARK: - Existence checks

    func resourcesExist() -> Bool {
        let base = Self.magentaHome.appendingPathComponent("resources")
        return FileManager.default.fileExists(
            atPath: base.appendingPathComponent("musiccoca/music_encoder.tflite").path)
            && FileManager.default.fileExists(
            atPath: base.appendingPathComponent("spectrostream/decoder.safetensors").path)
    }

    func modelExists(_ name: String) -> Bool {
        FileManager.default.fileExists(
            atPath: Self.magentaHome.appendingPathComponent("models/\(name)/\(name).mlxfn").path)
    }

    func modelPath(_ name: String) -> String {
        Self.magentaHome.appendingPathComponent("models/\(name)/\(name).mlxfn").path
    }

    // MARK: - Listing

    private func listFiles(path: String) async throws -> [HFFile] {
        let url = URL(string: "https://huggingface.co/api/models/\(repo)/tree/main/\(path)")!
        let (data, response) = try await URLSession.shared.data(from: url)
        guard (response as? HTTPURLResponse)?.statusCode == 200 else {
            throw NSError(domain: "ModelDownloader", code: 1, userInfo: [
                NSLocalizedDescriptionKey: "Failed to list \(path) on HuggingFace."
            ])
        }
        return try JSONDecoder().decode([HFFile].self, from: data)
    }

    // MARK: - Download orchestration

    /// Begin downloading `model` ("mrt2_small" or "mrt2_base"). If the shared
    /// resources (musiccoca, spectrostream) are not already present, they are
    /// queued first automatically.
    func startDownload(model: String) {
        guard !isDownloading else { return }
        isDownloading   = true
        errorMessage    = nil
        completedBytes  = 0
        overallProgress = 0
        currentModel    = model
        statusMessage   = "Fetching file list…"

        Task {
            do {
                var files: [HFFile] = []
                if !resourcesExist() {
                    async let musiccoca     = listFiles(path: "resources/musiccoca")
                    async let spectrostream = listFiles(path: "resources/spectrostream")
                    files += try await musiccoca
                    files += try await spectrostream
                }
                files += try await listFiles(path: "models/\(model)")

                self.totalBytes = files.reduce(0) { $0 + $1.size }
                self.queue = files
                self.downloadNext()
            } catch {
                self.statusMessage = "Failed to fetch file list"
                self.errorMessage  = error.localizedDescription
                self.isDownloading = false
            }
        }
    }

    func cancel() {
        session.invalidateAndCancel()
        isDownloading = false
        statusMessage = "Cancelled"
        // lazy `session` was invalidated — recreate on next use
        session = URLSession(configuration: .default, delegate: self, delegateQueue: nil)
    }

    private func downloadNext() {
        guard !queue.isEmpty else {
            statusMessage   = "Download complete"
            isDownloading   = false
            overallProgress = 1.0
            onComplete?(modelPath(currentModel))
            return
        }

        let file = queue.removeFirst()
        let dest = Self.magentaHome.appendingPathComponent(file.path)
        currentFileName = (file.path as NSString).lastPathComponent
        statusMessage   = "Downloading \(currentFileName)…"

        stateLock.lock()
        pendingDestination = dest
        pendingFileSize    = file.size
        stateLock.unlock()

        let url = URL(string: "https://huggingface.co/\(repo)/resolve/main/\(file.path)")!
        session.downloadTask(with: url).resume()
    }
}

// MARK: - URLSessionDownloadDelegate

extension ModelDownloader: URLSessionDownloadDelegate {

    // Fires frequently on a background delegate queue. `completedBytes` and
    // `totalBytes` are only ever mutated on the main actor and read here for
    // a progress percentage — a stale read just shows slightly-behind
    // progress for one frame, which is harmless (same tradeoff as
    // LevelTracker's RMS values; see docs/realtime-audio.md).
    nonisolated func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask,
                                 didWriteData bytesWritten: Int64, totalBytesWritten: Int64,
                                 totalBytesExpectedToWrite: Int64) {
        Task { @MainActor in
            let done = self.completedBytes + totalBytesWritten
            self.overallProgress = self.totalBytes > 0 ? Double(done) / Double(self.totalBytes) : 0
        }
    }

    // Fires once per file, on the delegate queue. The OS deletes the temp
    // file the instant this method returns, so the move must happen here
    // synchronously — it cannot be deferred to a MainActor Task.
    nonisolated func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask,
                                 didFinishDownloadingTo location: URL) {
        stateLock.lock()
        let dest = pendingDestination
        let size = pendingFileSize
        stateLock.unlock()

        guard let dest else { return }

        do {
            try FileManager.default.createDirectory(
                at: dest.deletingLastPathComponent(), withIntermediateDirectories: true)
            if FileManager.default.fileExists(atPath: dest.path) {
                try FileManager.default.removeItem(at: dest)
            }
            try FileManager.default.moveItem(at: location, to: dest)
        } catch {
            Task { @MainActor in
                self.errorMessage = "Failed to save \(dest.lastPathComponent): \(error.localizedDescription)"
                self.isDownloading = false
            }
            return
        }

        Task { @MainActor in
            self.completedBytes += size
            self.downloadNext()
        }
    }

    nonisolated func urlSession(_ session: URLSession, task: URLSessionTask,
                                 didCompleteWithError error: Error?) {
        guard let error, (error as NSError).code != NSURLErrorCancelled else { return }
        Task { @MainActor in
            self.errorMessage  = error.localizedDescription
            self.isDownloading = false
        }
    }
}
