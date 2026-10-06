import Foundation

enum MAIClient {
    static let model = "MAI-Transcribe-2"
    static let apiVersion = "2025-10-15"
    static let maxPhrases = 50

    static func apiKey() -> String? { CloudCredentials.value(.azureSpeechKey) }

    static func endpointURL() -> URL? {
        guard let raw = CloudCredentials.value(.azureSpeechEndpoint) else { return nil }
        let base = raw.contains("://") ? raw : "https://\(raw).cognitiveservices.azure.com"
        let trimmed = base.hasSuffix("/") ? String(base.dropLast()) : base
        return URL(
            string: "\(trimmed)/speechtotext/transcriptions:transcribe?api-version=\(apiVersion)")
    }

    static func transcribe(fileURL: URL) async throws -> String {
        guard let key = apiKey() else { throw MAIError.noKey }
        guard let url = endpointURL() else { throw MAIError.noEndpoint }

        let boundary = "SpeechToText-\(UUID().uuidString)"
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.timeoutInterval = 120
        request.setValue(key, forHTTPHeaderField: "Ocp-Apim-Subscription-Key")
        request.setValue(
            "multipart/form-data; boundary=\(boundary)", forHTTPHeaderField: "Content-Type")

        var body = Data()
        body.append(Data("--\(boundary)\r\n".utf8))
        body.append(Data(
            "Content-Disposition: form-data; name=\"audio\"; filename=\"audio.wav\"\r\n".utf8))
        body.append(Data("Content-Type: audio/wav\r\n\r\n".utf8))
        body.append(try Data(contentsOf: fileURL))
        body.append(Data("\r\n".utf8))
        body.append(Data("--\(boundary)\r\n".utf8))
        body.append(Data("Content-Disposition: form-data; name=\"definition\"\r\n\r\n".utf8))
        body.append(try definitionJSON())
        body.append(Data("\r\n".utf8))
        body.append(Data("--\(boundary)--\r\n".utf8))
        request.httpBody = body

        let (data, response) = try await URLSession.shared.data(for: request)
        guard let http = response as? HTTPURLResponse, http.statusCode == 200 else {
            throw MAIError.serverError(status: (response as? HTTPURLResponse)?.statusCode ?? -1)
        }
        guard let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
            let combined = json["combinedPhrases"] as? [[String: Any]]
        else { throw MAIError.badResponse }
        return combined.compactMap { $0["text"] as? String }
            .joined(separator: " ")
            .trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private static func definitionJSON() throws -> Data {
        var definition: [String: Any] = [
            "enhancedMode": [
                "enabled": true,
                "model": model,
                "modelOptions": [
                    "transcribeStyle": "clean",
                    "timestamps": "none",
                ],
            ]
        ]
        let phrases = Array(PersonalDictionary.terms().prefix(maxPhrases))
        if !phrases.isEmpty {
            definition["phraseList"] = ["phrases": phrases]
        }
        return try JSONSerialization.data(withJSONObject: definition)
    }

}

enum MAIError: LocalizedError {
    case noKey
    case noEndpoint
    case serverError(status: Int)
    case badResponse

    var errorDescription: String? {
        switch self {
        case .noKey:
            return "MAI-Transcribe-2 needs your Azure Speech key. Add it in Settings."
        case .noEndpoint:
            return "MAI-Transcribe-2 needs your Azure Speech endpoint. Add it in Settings."
        case .serverError(let status):
            return "MAI-Transcribe-2 request failed (\(status)). Internet up, key and endpoint right?"
        case .badResponse:
            return "MAI-Transcribe-2 returned an unreadable response. Audio saved to history."
        }
    }
}
