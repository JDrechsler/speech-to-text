import Foundation

struct WhisperModel: Identifiable, Equatable {
    let id: String
    let name: String
    let detail: String
    let bytes: Int64
    let isRecommended: Bool

    var fileName: String { "ggml-\(id).bin" }
    var fileURL: URL { AppPaths.modelsDirectory.appendingPathComponent(fileName) }
    var downloadURL: URL {
        URL(string: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/\(fileName)")!
    }
    var isInstalled: Bool { FileManager.default.fileExists(atPath: fileURL.path) }
    var sizeText: String { ByteCountFormatter.string(fromByteCount: bytes, countStyle: .file) }

    static let catalog: [WhisperModel] = [
        WhisperModel(
            id: "base", name: "Base",
            detail: "Fastest and smallest. Fine for quick notes in clear English.",
            bytes: 147_951_465, isRecommended: false),
        WhisperModel(
            id: "small", name: "Small",
            detail: "Better accuracy, still quick on any Apple Silicon Mac.",
            bytes: 487_601_967, isRecommended: false),
        WhisperModel(
            id: "large-v3-turbo-q5_0", name: "Large v3 Turbo (compact)",
            detail: "Near-best accuracy in 99 languages at a third of the download.",
            bytes: 574_041_195, isRecommended: true),
        WhisperModel(
            id: "large-v3-turbo", name: "Large v3 Turbo",
            detail: "Best accuracy. Needs about 2 GB of memory while the app runs.",
            bytes: 1_624_555_275, isRecommended: false),
    ]

    static let recommended = catalog.first(where: \.isRecommended)!

    static let selectionKey = "whisperModel"

    static var selected: WhisperModel {
        let stored = UserDefaults.standard.string(forKey: selectionKey)
        if let model = catalog.first(where: { $0.id == stored }) { return model }
        if recommended.isInstalled { return recommended }
        return catalog.last(where: \.isInstalled) ?? recommended
    }
}

enum WhisperLanguage {
    static let storageKey = "whisperLanguage"

    static let options: [(code: String, name: String)] = [
        ("auto", "Detect automatically"),
        ("en", "English"),
        ("de", "German"),
        ("es", "Spanish"),
        ("fr", "French"),
        ("it", "Italian"),
        ("pt", "Portuguese"),
        ("nl", "Dutch"),
        ("pl", "Polish"),
        ("tr", "Turkish"),
        ("uk", "Ukrainian"),
        ("ja", "Japanese"),
        ("zh", "Chinese"),
    ]

    static var current: String {
        UserDefaults.standard.string(forKey: storageKey) ?? "auto"
    }
}

@MainActor
final class WhisperModelStore: NSObject, ObservableObject {
    static let shared = WhisperModelStore()

    @Published private(set) var progress: [String: Double] = [:]
    @Published private(set) var failures: [String: String] = [:]
    @Published private(set) var installedIDs: Set<String> = []
    @Published var selectedID: String = WhisperModel.selected.id {
        didSet {
            UserDefaults.standard.set(selectedID, forKey: WhisperModel.selectionKey)
            if EnginePreference.stored == .whisper {
                LocalWhisper.shared.preload(selectedModel)
            } else {
                LocalWhisper.shared.unload()
            }
        }
    }

    private var tasks: [String: URLSessionDownloadTask] = [:]
    private lazy var session = URLSession(
        configuration: .default, delegate: self, delegateQueue: .main)

    private override init() {
        super.init()
        refresh()
    }

    var selectedModel: WhisperModel {
        WhisperModel.catalog.first { $0.id == selectedID } ?? WhisperModel.recommended
    }

    func isDownloading(_ model: WhisperModel) -> Bool { tasks[model.id] != nil }

    func download(_ model: WhisperModel) {
        guard tasks[model.id] == nil, !model.isInstalled else { return }
        failures[model.id] = nil
        progress[model.id] = 0
        let task = session.downloadTask(with: model.downloadURL)
        task.taskDescription = model.id
        tasks[model.id] = task
        task.resume()
    }

    func cancel(_ model: WhisperModel) {
        tasks[model.id]?.cancel()
        tasks[model.id] = nil
        progress[model.id] = nil
    }

    func delete(_ model: WhisperModel) {
        if selectedID == model.id { LocalWhisper.shared.unload() }
        try? FileManager.default.removeItem(at: model.fileURL)
        refresh()
    }

    func refresh() {
        installedIDs = Set(WhisperModel.catalog.filter(\.isInstalled).map(\.id))
    }

    private func finish(_ modelID: String, failure: String?) {
        tasks[modelID] = nil
        progress[modelID] = nil
        failures[modelID] = failure
        refresh()
        if failure == nil, !selectedModel.isInstalled {
            selectedID = modelID
        }
    }
}

extension WhisperModelStore: URLSessionDownloadDelegate {
    nonisolated func urlSession(
        _ session: URLSession, downloadTask: URLSessionDownloadTask, didWriteData bytesWritten: Int64,
        totalBytesWritten: Int64, totalBytesExpectedToWrite: Int64
    ) {
        guard let id = downloadTask.taskDescription,
            let model = WhisperModel.catalog.first(where: { $0.id == id })
        else { return }
        let expected = totalBytesExpectedToWrite > 0 ? totalBytesExpectedToWrite : model.bytes
        let fraction = min(1, Double(totalBytesWritten) / Double(expected))
        MainActor.assumeIsolated {
            if tasks[id] != nil { progress[id] = fraction }
        }
    }

    nonisolated func urlSession(
        _ session: URLSession, downloadTask: URLSessionDownloadTask,
        didFinishDownloadingTo location: URL
    ) {
        guard let id = downloadTask.taskDescription,
            let model = WhisperModel.catalog.first(where: { $0.id == id })
        else { return }
        let failure = Self.install(download: location, response: downloadTask.response, as: model)
        MainActor.assumeIsolated { finish(id, failure: failure) }
    }

    nonisolated func urlSession(
        _ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?
    ) {
        guard let error, let id = task.taskDescription else { return }
        if (error as? URLError)?.code == .cancelled { return }
        MainActor.assumeIsolated {
            finish(id, failure: "Download failed: \(error.localizedDescription)")
        }
    }

    private nonisolated static func install(
        download location: URL, response: URLResponse?, as model: WhisperModel
    ) -> String? {
        let status = (response as? HTTPURLResponse)?.statusCode ?? -1
        guard status == 200 else { return "Download failed (HTTP \(status)). Try again later." }
        let fileManager = FileManager.default
        let size = (try? fileManager.attributesOfItem(atPath: location.path)[.size] as? Int64) ?? 0
        guard size == model.bytes else {
            return "Download incomplete (\(size) of \(model.bytes) bytes). Try again."
        }
        do {
            try fileManager.createDirectory(
                at: AppPaths.modelsDirectory, withIntermediateDirectories: true)
            if fileManager.fileExists(atPath: model.fileURL.path) {
                try fileManager.removeItem(at: model.fileURL)
            }
            try fileManager.moveItem(at: location, to: model.fileURL)
            return nil
        } catch {
            return "Could not save the model: \(error.localizedDescription)"
        }
    }
}
