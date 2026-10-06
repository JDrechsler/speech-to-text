import Foundation

struct CredentialField: Identifiable, Hashable {
    let id: String
    let label: String
    let placeholder: String
    let isSecret: Bool

    static let azureSpeechKey = CredentialField(
        id: "AZURE_SPEECH_KEY", label: "API key", placeholder: "Key 1 from your Azure Speech resource",
        isSecret: true)
    static let azureSpeechEndpoint = CredentialField(
        id: "AZURE_SPEECH_ENDPOINT", label: "Endpoint",
        placeholder: "https://<resource-name>.cognitiveservices.azure.com", isSecret: false)
    static let elevenLabsKey = CredentialField(
        id: "ELEVENLABS_API_KEY", label: "API key", placeholder: "ElevenLabs API key", isSecret: true)
    static let assemblyKey = CredentialField(
        id: "ASSEMBLYAI_API_KEY", label: "API key", placeholder: "AssemblyAI API key", isSecret: true)
}

@MainActor
final class CloudCredentials: ObservableObject {
    static let shared = CloudCredentials()

    @Published private(set) var savedFieldIDs: Set<String> = []

    private init() {
        refresh()
    }

    func isSet(_ field: CredentialField) -> Bool {
        savedFieldIDs.contains(field.id)
    }

    func save(_ value: String, for field: CredentialField) -> Bool {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return false }
        let saved: Bool
        if field.isSecret {
            saved = Keychain.save(trimmed, for: field.id)
        } else {
            UserDefaults.standard.set(trimmed, forKey: field.id)
            saved = true
        }
        refresh()
        return saved
    }

    func remove(_ field: CredentialField) {
        if field.isSecret {
            Keychain.delete(field.id)
        } else {
            UserDefaults.standard.removeObject(forKey: field.id)
        }
        refresh()
    }

    func refresh() {
        let all = EnginePreference.cloud.flatMap(\.credentialFields)
        savedFieldIDs = Set(all.filter(Self.storedValueExists).map(\.id))
    }

    nonisolated static func value(_ field: CredentialField) -> String? {
        if field.isSecret { return Keychain.read(field.id) }
        let value = UserDefaults.standard.string(forKey: field.id)?
            .trimmingCharacters(in: .whitespacesAndNewlines)
        return value?.isEmpty == false ? value : nil
    }

    nonisolated static func publicValue(_ field: CredentialField) -> String? {
        field.isSecret ? nil : value(field)
    }

    private static func storedValueExists(_ field: CredentialField) -> Bool {
        field.isSecret ? Keychain.contains(field.id) : value(field) != nil
    }
}
