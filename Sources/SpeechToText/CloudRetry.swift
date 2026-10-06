import Foundation

enum CloudRetry {
    static let attempts = 3
    static let backoffSeconds: [Double] = [1, 2]

    static func run(
        engine: EnginePreference,
        recordingSeconds: Double,
        onAttempt: @MainActor (Int) -> Void,
        _ transcribe: () async throws -> String
    ) async throws -> String {
        var attempt = 1
        while true {
            await onAttempt(attempt)
            let started = Date()
            do {
                let text = try await transcribe()
                if attempt > 1 {
                    AppLog.write([
                        "event": "recovered", "engine": engine.rawValue,
                        "attempt": "\(attempt)/\(attempts)",
                        "took": seconds(since: started),
                    ])
                }
                return text
            } catch {
                let transient = isTransient(error)
                AppLog.write([
                    "event": "cloud_error", "engine": engine.rawValue,
                    "attempt": "\(attempt)/\(attempts)",
                    "recording": String(format: "%.1fs", recordingSeconds),
                    "took": seconds(since: started),
                    "transient": transient ? "yes" : "no",
                    "code": errorCode(error),
                    "error": error.localizedDescription,
                ])
                guard transient, attempt < attempts else { throw error }
                let pause = backoffSeconds[min(attempt - 1, backoffSeconds.count - 1)]
                try? await Task.sleep(nanoseconds: UInt64(pause * 1_000_000_000))
                attempt += 1
            }
        }
    }

    static func isTransient(_ error: Error) -> Bool {
        if let urlError = error as? URLError {
            return transientURLErrorCodes.contains(urlError.code)
        }
        switch error {
        case MAIError.serverError(let status): return isTransientStatus(status)
        case ScribeError.serverError(let status): return isTransientStatus(status)
        case AssemblyError.serverError: return true
        case StepFunError.serverError(let status): return isTransientStatus(status)
        default: return false
        }
    }

    private static let transientURLErrorCodes: Set<URLError.Code> = [
        .timedOut, .networkConnectionLost, .cannotConnectToHost, .cannotFindHost,
        .dnsLookupFailed, .notConnectedToInternet, .secureConnectionFailed,
        .resourceUnavailable, .internationalRoamingOff, .callIsActive,
        .dataNotAllowed,
    ]

    private static func isTransientStatus(_ status: Int) -> Bool {
        status == -1 || status == 408 || status == 429 || (500...599).contains(status)
    }

    private static func errorCode(_ error: Error) -> String {
        if let urlError = error as? URLError { return "URLError\(urlError.code.rawValue)" }
        switch error {
        case MAIError.serverError(let status), ScribeError.serverError(let status),
            StepFunError.serverError(let status):
            return "HTTP\(status)"
        default: return String(describing: type(of: error))
        }
    }

    private static func seconds(since date: Date) -> String {
        String(format: "%.1fs", Date().timeIntervalSince(date))
    }
}
