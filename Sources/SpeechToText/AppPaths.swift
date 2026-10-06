import Foundation

enum AppPaths {
    static let supportDirectory: URL = {
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        return base.appendingPathComponent("SpeechToText", isDirectory: true)
    }()

    static let modelsDirectory = ensured(supportDirectory.appendingPathComponent("models", isDirectory: true))

    static let recordingsDirectory = ensured(
        supportDirectory.appendingPathComponent("recordings", isDirectory: true))

    static let dictionaryFile = supportDirectory.appendingPathComponent("dictionary.txt")

    static let logsDirectory: URL = {
        let library = FileManager.default.urls(for: .libraryDirectory, in: .userDomainMask)[0]
        return library.appendingPathComponent("Logs/SpeechToText", isDirectory: true)
    }()

    private static func ensured(_ directory: URL) -> URL {
        try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        return directory
    }
}
