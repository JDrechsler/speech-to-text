import Foundation

enum AssemblyClient {
    static func apiKey() -> String? { CloudCredentials.value(.assemblyKey) }

    static func transcribe(fileURL: URL) async throws -> String {
        guard let key = apiKey() else { throw AssemblyError.noKey }

        // 1. Upload the audio bytes.
        var upload = URLRequest(url: URL(string: "https://api.assemblyai.com/v2/upload")!)
        upload.httpMethod = "POST"
        upload.timeoutInterval = 120
        upload.setValue(key, forHTTPHeaderField: "authorization")
        upload.httpBody = try Data(contentsOf: fileURL)
        let (upData, upResp) = try await URLSession.shared.data(for: upload)
        guard (upResp as? HTTPURLResponse)?.statusCode == 200,
            let upJSON = try? JSONSerialization.jsonObject(with: upData) as? [String: Any],
            let audioURL = upJSON["upload_url"] as? String
        else { throw AssemblyError.serverError(step: "upload") }

        // 2. Create the transcription job.
        var create = URLRequest(url: URL(string: "https://api.assemblyai.com/v2/transcript")!)
        create.httpMethod = "POST"
        create.timeoutInterval = 30
        create.setValue(key, forHTTPHeaderField: "authorization")
        create.setValue("application/json", forHTTPHeaderField: "Content-Type")
        var jobBody: [String: Any] = [
            "audio_url": audioURL,
            "speech_models": ["universal-3-5-pro"],
            "language_detection": true,
        ]
        let terms = Array(PersonalDictionary.terms().prefix(100))
        if !terms.isEmpty { jobBody["keyterms_prompt"] = terms }
        create.httpBody = try JSONSerialization.data(withJSONObject: jobBody)
        let (crData, crResp) = try await URLSession.shared.data(for: create)
        guard (crResp as? HTTPURLResponse)?.statusCode == 200,
            let crJSON = try? JSONSerialization.jsonObject(with: crData) as? [String: Any],
            let jobID = crJSON["id"] as? String
        else { throw AssemblyError.serverError(step: "create") }

        // 3. Poll until done, quickly at first because short dictations finish in about a second.
        let statusURL = URL(string: "https://api.assemblyai.com/v2/transcript/\(jobID)")!
        for attempt in 0..<120 {
            try await Task.sleep(nanoseconds: attempt < 8 ? 350_000_000 : 1_500_000_000)
            var poll = URLRequest(url: statusURL)
            poll.timeoutInterval = 30
            poll.setValue(key, forHTTPHeaderField: "authorization")
            let (pData, _) = try await URLSession.shared.data(for: poll)
            guard let pJSON = try? JSONSerialization.jsonObject(with: pData) as? [String: Any],
                let status = pJSON["status"] as? String
            else { continue }
            if status == "completed" {
                return (pJSON["text"] as? String ?? "")
                    .trimmingCharacters(in: .whitespacesAndNewlines)
            }
            if status == "error" {
                throw AssemblyError.jobFailed(pJSON["error"] as? String ?? "unknown")
            }
        }
        throw AssemblyError.timedOut
    }
}

enum AssemblyError: LocalizedError {
    case noKey
    case serverError(step: String)
    case jobFailed(String)
    case timedOut

    var errorDescription: String? {
        switch self {
        case .noKey:
            return "AssemblyAI needs your API key. Add it in Settings."
        case .serverError(let step):
            return "AssemblyAI \(step) failed. Internet up, key right?"
        case .jobFailed(let message):
            return "AssemblyAI error: \(message)"
        case .timedOut:
            return "AssemblyAI timed out. Audio saved to history."
        }
    }
}
