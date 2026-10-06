import Foundation

enum PersonalDictionary {
    static let maxTermLength = 50

    static var fileURL: URL { AppPaths.dictionaryFile }

    private static let template = """
        # Your personal dictionary for Speech to Text. It lives only on this Mac.
        # Add one word or name per line, spelled exactly the way you want it written.
        # Every engine gets these as spelling hints. Cloud engines receive them with each recording.
        # Keep it under 50 entries: rare names and jargon help, common words do not.
        # Lines starting with # are ignored.

        """

    static func ensureFileExists() {
        guard !FileManager.default.fileExists(atPath: fileURL.path) else { return }
        try? FileManager.default.createDirectory(
            at: fileURL.deletingLastPathComponent(), withIntermediateDirectories: true)
        try? template.write(to: fileURL, atomically: true, encoding: .utf8)
    }

    static func terms() -> [String] {
        guard let content = try? String(contentsOf: fileURL, encoding: .utf8) else { return [] }
        return content.split(whereSeparator: \.isNewline)
            .map { $0.trimmingCharacters(in: .whitespaces) }
            .filter { !$0.isEmpty && !$0.hasPrefix("#") && $0.count <= maxTermLength }
    }
}
