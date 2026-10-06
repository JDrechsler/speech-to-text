import Foundation

enum AppLog {
    static let fileURL = AppPaths.logsDirectory.appendingPathComponent("app.log")

    private static let maxBytes = 1_000_000
    private static let queue = DispatchQueue(label: "SpeechToText.AppLog")

    static func write(_ fields: [String: String]) {
        let stamp = ISO8601DateFormatter().string(from: Date())
        let line = ([stamp] + fields.sorted { $0.key < $1.key }.map { "\($0.key)=\(quote($0.value))" })
            .joined(separator: " ") + "\n"
        queue.async {
            rotateIfNeeded()
            append(line)
        }
    }

    private static func quote(_ value: String) -> String {
        value.contains(" ") ? "\"\(value.replacingOccurrences(of: "\"", with: "'"))\"" : value
    }

    private static func append(_ line: String) {
        let fm = FileManager.default
        try? fm.createDirectory(
            at: fileURL.deletingLastPathComponent(), withIntermediateDirectories: true)
        if !fm.fileExists(atPath: fileURL.path) {
            fm.createFile(atPath: fileURL.path, contents: nil)
        }
        guard let handle = try? FileHandle(forWritingTo: fileURL) else { return }
        defer { try? handle.close() }
        _ = try? handle.seekToEnd()
        try? handle.write(contentsOf: Data(line.utf8))
    }

    private static func rotateIfNeeded() {
        let fm = FileManager.default
        guard let size = try? fm.attributesOfItem(atPath: fileURL.path)[.size] as? Int,
            size > maxBytes
        else { return }
        let rotated = fileURL.deletingPathExtension().appendingPathExtension("log.1")
        try? fm.removeItem(at: rotated)
        try? fm.moveItem(at: fileURL, to: rotated)
    }
}
