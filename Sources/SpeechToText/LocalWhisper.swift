import AVFoundation
import Foundation
import whisper

final class LocalWhisper: @unchecked Sendable {
    static let shared = LocalWhisper()

    private static let threadCount: Int32 = 4
    private static let sampleRate = 16_000.0

    private let queue = DispatchQueue(label: "SpeechToText.whisper", qos: .userInitiated)
    private var context: OpaquePointer?
    private var loadedModelPath: String?

    func preload(_ model: WhisperModel) {
        guard model.isInstalled else { return }
        queue.async { _ = try? self.loadContext(for: model) }
    }

    func unload() {
        queue.async { self.freeContext() }
    }

    func releaseBeforeExit() {
        queue.sync { freeContext() }
    }

    func transcribe(fileURL: URL, model: WhisperModel, language: String, prompt: String?)
        async throws -> String
    {
        guard model.isInstalled else { throw LocalWhisperError.modelMissing(model.name) }
        return try await withCheckedThrowingContinuation { continuation in
            queue.async {
                do {
                    let text = try self.run(
                        fileURL: fileURL, model: model, language: language, prompt: prompt)
                    continuation.resume(returning: text)
                } catch {
                    continuation.resume(throwing: error)
                }
            }
        }
    }

    private func run(fileURL: URL, model: WhisperModel, language: String, prompt: String?) throws
        -> String
    {
        let context = try loadContext(for: model)
        let samples = try Self.readSamples(fileURL)
        guard !samples.isEmpty else { return "" }

        var params = whisper_full_default_params(WHISPER_SAMPLING_GREEDY)
        params.n_threads = Self.threadCount
        params.translate = false
        params.no_timestamps = true
        params.print_progress = false
        params.print_realtime = false
        params.print_special = false
        params.print_timestamps = false
        params.temperature = 0

        let status = language.withCString { languagePointer in
            (prompt ?? "").withCString { promptPointer in
                params.language = languagePointer
                params.initial_prompt = prompt == nil ? nil : promptPointer
                return samples.withUnsafeBufferPointer { buffer in
                    whisper_full(context, params, buffer.baseAddress, Int32(buffer.count))
                }
            }
        }
        guard status == 0 else { throw LocalWhisperError.inferenceFailed(status) }

        let segments = (0..<whisper_full_n_segments(context)).compactMap { index in
            whisper_full_get_segment_text(context, index).map { String(cString: $0) }
        }
        return segments.joined(separator: " ")
            .replacingOccurrences(of: "\\s+", with: " ", options: .regularExpression)
            .trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private func loadContext(for model: WhisperModel) throws -> OpaquePointer {
        if let context, loadedModelPath == model.fileURL.path { return context }
        freeContext()
        var params = whisper_context_default_params()
        params.use_gpu = true
        params.flash_attn = false
        guard let loaded = whisper_init_from_file_with_params(model.fileURL.path, params) else {
            throw LocalWhisperError.modelLoadFailed(model.name)
        }
        context = loaded
        loadedModelPath = model.fileURL.path
        return loaded
    }

    private func freeContext() {
        if let context { whisper_free(context) }
        context = nil
        loadedModelPath = nil
    }

    private static func readSamples(_ fileURL: URL) throws -> [Float] {
        let file = try AVAudioFile(
            forReading: fileURL, commonFormat: .pcmFormatFloat32, interleaved: false)
        let format = file.processingFormat
        guard format.sampleRate == sampleRate, format.channelCount == 1 else {
            throw LocalWhisperError.unsupportedAudio
        }
        guard file.length > 0,
            let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: AVAudioFrameCount(file.length))
        else { return [] }
        try file.read(into: buffer)
        guard let channel = buffer.floatChannelData?[0] else { return [] }
        return Array(UnsafeBufferPointer(start: channel, count: Int(buffer.frameLength)))
    }
}

enum LocalWhisperError: LocalizedError {
    case modelMissing(String)
    case modelLoadFailed(String)
    case inferenceFailed(Int32)
    case unsupportedAudio

    var errorDescription: String? {
        switch self {
        case .modelMissing(let name):
            return "Whisper model \(name) is not downloaded. Open Settings to download it."
        case .modelLoadFailed(let name):
            return "Could not load Whisper model \(name). Delete and download it again in Settings."
        case .inferenceFailed(let status):
            return "Whisper failed to transcribe (code \(status)). Audio saved to history."
        case .unsupportedAudio:
            return "Whisper needs a 16 kHz mono recording made by this app."
        }
    }
}
