import Foundation

enum StepFunClient {
    static let model = "stepaudio-3-asr-max"
    static let endpoint = URL(string: "https://api.stepfun.ai/v1/audio/asr/sse")!
    static let languageKey = "stepFunLanguage"

    static let languages: [(code: String, name: String)] = [
        ("en", "English"),
        ("es", "Spanish"),
        ("fr", "French"),
        ("zh", "Chinese"),
        ("ja", "Japanese"),
        ("ko", "Korean"),
    ]

    static var language: String {
        UserDefaults.standard.string(forKey: languageKey) ?? "en"
    }

    static func transcribe(fileURL: URL) async throws -> String {
        guard let key = CloudCredentials.value(.stepFunKey) else { throw StepFunError.noKey }

        var request = URLRequest(url: endpoint)
        request.httpMethod = "POST"
        request.timeoutInterval = 120
        request.setValue("Bearer \(key)", forHTTPHeaderField: "Authorization")
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("text/event-stream", forHTTPHeaderField: "Accept")
        let body: [String: Any] = [
            "audio": [
                "data": try Data(contentsOf: fileURL).base64EncodedString(),
                "input": [
                    "transcription": [
                        "model": model,
                        "language": language,
                        "enable_itn": true,
                    ],
                    "format": ["type": "wav"],
                ],
            ]
        ]
        request.httpBody = try JSONSerialization.data(withJSONObject: body)

        let (data, response) = try await URLSession.shared.data(for: request)
        let status = (response as? HTTPURLResponse)?.statusCode ?? -1
        guard status == 200 else { throw StepFunError.serverError(status: status) }
        return try transcript(fromEventStream: data)
    }

    private static func transcript(fromEventStream data: Data) throws -> String {
        let lines = String(decoding: data, as: UTF8.self).split(whereSeparator: \.isNewline)
        for line in lines where line.hasPrefix("data:") {
            let payload = line.dropFirst("data:".count).trimmingCharacters(in: .whitespaces)
            guard let event = try? JSONSerialization.jsonObject(with: Data(payload.utf8)) as? [String: Any]
            else { continue }
            switch event["type"] as? String {
            case "transcript.text.done":
                return (event["text"] as? String ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
            case "error":
                throw StepFunError.recognitionFailed(event["message"] as? String ?? "unknown error")
            default:
                continue
            }
        }
        throw StepFunError.badResponse
    }
}

enum StepFunError: LocalizedError {
    case noKey
    case serverError(status: Int)
    case recognitionFailed(String)
    case badResponse

    var errorDescription: String? {
        switch self {
        case .noKey:
            return "StepFun needs your API key. Add it in Settings."
        case .serverError(401):
            return "StepFun rejected the API key. Check it in Settings."
        case .serverError(402):
            return "StepFun says your account balance is too low."
        case .serverError(let status):
            return "StepFun request failed (\(status)). Internet up, key right?"
        case .recognitionFailed(let message):
            return "StepFun error: \(message)"
        case .badResponse:
            return "StepFun returned no transcript. Audio saved to history."
        }
    }
}
