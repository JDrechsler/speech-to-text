import Foundation

struct RecordingEntry: Identifiable, Codable, Equatable {
    /// Base file name without extension, e.g. "2026-07-13_17-02-41".
    let id: String
    let createdAt: Date
    var duration: TimeInterval
    var transcript: String
    var engine: String

    var fileName: String { "\(id).wav" }
}

/// Recordings live in ~/Library/Application Support/SpeechToText/recordings.
/// Each WAV gets a JSON sidecar with the transcript — one file pair per
/// recording, so nothing can be lost to a corrupted central index.
@MainActor
final class HistoryStore: ObservableObject {
    @Published private(set) var entries: [RecordingEntry] = []

    static let recordingsDirectory = AppPaths.recordingsDirectory

    private static let fileNameFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.dateFormat = "yyyy-MM-dd_HH-mm-ss"
        formatter.locale = Locale(identifier: "en_US_POSIX")
        return formatter
    }()

    /// Recordings older than this are deleted by `cleanupExpired()`.
    static let retentionInterval: TimeInterval = 24 * 60 * 60

    init() {
        cleanupExpired()
    }

    /// Permanently removes recordings (WAV + sidecar) older than the
    /// retention window. Runs at launch and hourly.
    func cleanupExpired() {
        let cutoff = Date().addingTimeInterval(-Self.retentionInterval)
        let fileManager = FileManager.default
        let files =
            (try? fileManager.contentsOfDirectory(
                at: Self.recordingsDirectory,
                includingPropertiesForKeys: [.creationDateKey])) ?? []

        for file in files where file.pathExtension == "wav" {
            let created =
                (try? file.resourceValues(forKeys: [.creationDateKey]))?.creationDate ?? Date()
            guard created < cutoff else { continue }
            let id = file.deletingPathExtension().lastPathComponent
            try? fileManager.removeItem(at: file)
            try? fileManager.removeItem(at: sidecarURL(id: id))
        }
        reload()
    }

    func newRecordingURL() -> URL {
        var name = Self.fileNameFormatter.string(from: Date())
        // Avoid collision if two recordings start within the same second.
        var url = Self.recordingsDirectory.appendingPathComponent("\(name).wav")
        var suffix = 2
        while FileManager.default.fileExists(atPath: url.path) {
            name = "\(Self.fileNameFormatter.string(from: Date()))_\(suffix)"
            url = Self.recordingsDirectory.appendingPathComponent("\(name).wav")
            suffix += 1
        }
        return url
    }

    func saveEntry(audioURL: URL, transcript: String, engine: String, duration: TimeInterval) {
        let id = audioURL.deletingPathExtension().lastPathComponent
        let entry = RecordingEntry(
            id: id, createdAt: Date(), duration: duration,
            transcript: transcript, engine: engine)
        writeSidecar(entry)
        reload()
    }

    func updateTranscript(id: String, transcript: String, engine: String) {
        guard var entry = entries.first(where: { $0.id == id }) else { return }
        entry.transcript = transcript
        entry.engine = engine
        writeSidecar(entry)
        reload()
    }

    func delete(_ entry: RecordingEntry) {
        let audio = Self.recordingsDirectory.appendingPathComponent(entry.fileName)
        let sidecar = sidecarURL(id: entry.id)
        try? FileManager.default.trashItem(at: audio, resultingItemURL: nil)
        try? FileManager.default.removeItem(at: sidecar)
        reload()
    }

    /// Scan the directory: every WAV is an entry, sidecar or not, so audio
    /// from a crashed session still shows up and can be re-transcribed.
    func reload() {
        let fileManager = FileManager.default
        guard
            let files = try? fileManager.contentsOfDirectory(
                at: Self.recordingsDirectory, includingPropertiesForKeys: [.creationDateKey])
        else {
            entries = []
            return
        }

        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601

        var result: [RecordingEntry] = []
        for file in files where file.pathExtension == "wav" {
            let id = file.deletingPathExtension().lastPathComponent
            if let data = try? Data(contentsOf: sidecarURL(id: id)),
                let entry = try? decoder.decode(RecordingEntry.self, from: data)
            {
                result.append(entry)
            } else {
                let created =
                    (try? file.resourceValues(forKeys: [.creationDateKey]))?.creationDate
                    ?? Date()
                result.append(
                    RecordingEntry(
                        id: id, createdAt: created, duration: 0,
                        transcript: "", engine: "none"))
            }
        }
        entries = result.sorted { $0.createdAt > $1.createdAt }
    }

    private func sidecarURL(id: String) -> URL {
        Self.recordingsDirectory.appendingPathComponent("\(id).json")
    }

    private func writeSidecar(_ entry: RecordingEntry) {
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        if let data = try? encoder.encode(entry) {
            try? data.write(to: sidecarURL(id: entry.id), options: .atomic)
        }
    }
}
