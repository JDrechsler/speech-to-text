import Foundation

enum ScribeClient {
    static func apiKey() -> String? { CloudCredentials.value(.elevenLabsKey) }

    static func transcribe(fileURL: URL) async throws -> String {
        guard let key = apiKey() else { throw ScribeError.noKey }

        let boundary = "SpeechToText-\(UUID().uuidString)"
        var request = URLRequest(url: URL(string: "https://api.elevenlabs.io/v1/speech-to-text")!)
        request.httpMethod = "POST"
        request.timeoutInterval = 120
        request.setValue(key, forHTTPHeaderField: "xi-api-key")
        request.setValue(
            "multipart/form-data; boundary=\(boundary)", forHTTPHeaderField: "Content-Type")

        var body = Data()
        func field(_ name: String, _ value: String) {
            body.append(Data("--\(boundary)\r\n".utf8))
            body.append(Data("Content-Disposition: form-data; name=\"\(name)\"\r\n\r\n".utf8))
            body.append(Data("\(value)\r\n".utf8))
        }
        body.append(Data("--\(boundary)\r\n".utf8))
        body.append(Data(
            "Content-Disposition: form-data; name=\"file\"; filename=\"audio.wav\"\r\n".utf8))
        body.append(Data("Content-Type: audio/wav\r\n\r\n".utf8))
        body.append(try Data(contentsOf: fileURL))
        body.append(Data("\r\n".utf8))
        field("model_id", "scribe_v2")
        for term in PersonalDictionary.terms() { field("keyterms", term) }
        body.append(Data("--\(boundary)--\r\n".utf8))
        request.httpBody = body

        let (data, response) = try await URLSession.shared.data(for: request)
        guard let http = response as? HTTPURLResponse, http.statusCode == 200 else {
            throw ScribeError.serverError(status: (response as? HTTPURLResponse)?.statusCode ?? -1)
        }
        guard let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
            let text = json["text"] as? String
        else { throw ScribeError.badResponse }
        return text.trimmingCharacters(in: .whitespacesAndNewlines)
    }
}

enum ScribeError: LocalizedError {
    case noKey
    case serverError(status: Int)
    case badResponse

    var errorDescription: String? {
        switch self {
        case .noKey:
            return "Scribe needs your ElevenLabs API key. Add it in Settings."
        case .serverError(let status):
            return "Scribe request failed (\(status)). Internet up, key right?"
        case .badResponse:
            return "Scribe returned an unreadable response. Audio saved to history."
        }
    }
}
