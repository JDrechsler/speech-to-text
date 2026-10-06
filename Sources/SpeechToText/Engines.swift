import Foundation

enum EnginePreference: String, CaseIterable, Identifiable {
    case whisper, mai, scribe, assembly

    static let local: [EnginePreference] = allCases.filter(\.isLocal)
    static let cloud: [EnginePreference] = allCases.filter { !$0.isLocal }
    static let firstLaunchDefault: EnginePreference = .whisper
    static let storageKey = "enginePreference"

    static let lastCloudStorageKey = "lastCloudEngine"

    @MainActor static var preferredCloud: EnginePreference {
        let last = UserDefaults.standard.string(forKey: lastCloudStorageKey)
        if let engine = cloud.first(where: { $0.rawValue == last }) { return engine }
        return cloud.first(where: \.isReady) ?? cloud[0]
    }

    static var stored: EnginePreference {
        guard let raw = UserDefaults.standard.string(forKey: storageKey) else {
            return firstLaunchDefault
        }
        return EnginePreference(rawValue: raw) ?? .whisper
    }

    var id: String { rawValue }

    var isLocal: Bool { self == .whisper }

    var displayName: String {
        switch self {
        case .whisper: return "Whisper"
        case .mai: return "MAI-Transcribe-2"
        case .scribe: return "Scribe v2"
        case .assembly: return "Universal-3.5 Pro"
        }
    }

    var provider: String {
        switch self {
        case .whisper: return "whisper.cpp"
        case .mai: return "Microsoft Azure"
        case .scribe: return "ElevenLabs"
        case .assembly: return "AssemblyAI"
        }
    }

    var shortName: String {
        switch self {
        case .whisper: return "Whisper"
        case .mai: return "MAI"
        case .scribe: return "Scribe"
        case .assembly: return "AssemblyAI"
        }
    }

    var summary: String {
        switch self {
        case .whisper:
            return "OpenAI's open Whisper model running on your Mac's GPU. Works without internet."
        case .mai:
            return "Microsoft's MAI model on Azure Speech. Clean punctuation, detects the language by itself."
        case .scribe:
            return "The fastest cloud engine in this app. Mixed-language recordings can lose the minority language."
        case .assembly:
            return "Verbatim and strong with several languages in one recording. A few seconds slower."
        }
    }

    var privacyBadge: String {
        isLocal ? "On this Mac" : "Cloud · \(provider)"
    }

    var shortPrivacyBadge: String {
        isLocal ? "Offline" : "Uploads audio"
    }

    var privacyStatement: String {
        isLocal
            ? "Audio never leaves your Mac."
            : "Each recording is uploaded to \(provider) for transcription."
    }

    var menuTitle: String {
        isLocal ? "\(displayName) · on this Mac" : "\(displayName) · \(provider)"
    }

    var historyLabel: String {
        switch self {
        case .whisper: return "whisper-\(WhisperModel.selected.id)"
        case .mai: return "mai-transcribe-2"
        case .scribe: return "scribe-v2"
        case .assembly: return "assemblyai-universal-3.5-pro"
        }
    }

    static func from(historyLabel: String) -> EnginePreference? {
        if historyLabel.hasPrefix("whisper") { return .whisper }
        return cloud.first { $0.historyLabel == historyLabel }
    }

    var credentialFields: [CredentialField] {
        switch self {
        case .whisper: return []
        case .mai: return [.azureSpeechKey, .azureSpeechEndpoint]
        case .scribe: return [.elevenLabsKey]
        case .assembly: return [.assemblyKey]
        }
    }

    var keyHelpURL: URL? {
        switch self {
        case .whisper: return nil
        case .mai: return URL(string: "https://learn.microsoft.com/azure/ai-services/speech-service/mai-transcribe")
        case .scribe: return URL(string: "https://elevenlabs.io/app/developers/api-keys")
        case .assembly: return URL(string: "https://www.assemblyai.com/dashboard/api-keys")
        }
    }

    var privacyPolicyURL: URL? {
        switch self {
        case .whisper: return nil
        case .mai:
            return URL(
                string:
                    "https://learn.microsoft.com/azure/foundry/responsible-ai/speech-service/speech-to-text/data-privacy-security")
        case .scribe: return URL(string: "https://elevenlabs.io/privacy-policy")
        case .assembly: return URL(string: "https://www.assemblyai.com/legal/privacy-policy")
        }
    }

    @MainActor var isReady: Bool {
        if isLocal { return WhisperModelStore.shared.selectedModel.isInstalled }
        return credentialFields.allSatisfy { CloudCredentials.shared.isSet($0) }
    }

    @MainActor var setupHint: String {
        if isLocal {
            return "No Whisper model downloaded yet. Open Settings and download one."
        }
        return "\(displayName) needs your \(provider) API key. Add it in Settings."
    }
}
