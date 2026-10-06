import AppKit

/// Pauses whatever media is playing while recording and resumes it after.
///
/// Uses the bundled mediaremote-adapter (vendor/mediaremote-adapter, BSD-3):
/// MediaRemote framework calls hosted inside Apple-signed /usr/bin/perl,
/// which restores the now-playing APIs Apple locked down in macOS 15.4+.
/// This covers everything macOS shows as "Now Playing" — Chrome/YouTube
/// tabs, Spotify, Music, podcasts, all of it.
///
/// Semantics (deliberately simple):
///  - On record start: send a true PAUSE command, unconditionally. Pause is
///    not a toggle — if nothing is playing it does nothing, so recording
///    can never accidentally START playback.
///  - On record end: send PLAY only if something was actually playing when
///    the recording started (state is queried, not guessed).
@MainActor
enum MediaController {
    private static var wasPlaying = false

    // MRCommand IDs understood by the adapter's `send`.
    private static let commandPlay = "0"
    private static let commandPause = "1"

    /// Pause now-playing media. Returns true when something was playing
    /// (callers give the audio a beat to fade before opening the mic).
    static func pauseForRecording() async -> Bool {
        wasPlaying = false
        guard adapterScript != nil else { return false }

        if let json = await runAdapter(["get", "--no-artwork"]),
            let data = json.data(using: .utf8),
            let info = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
            info["playing"] as? Bool == true
        {
            wasPlaying = true
        }
        _ = await runAdapter(["send", Self.commandPause])
        return wasPlaying
    }

    /// Resume only when the recording actually interrupted playback.
    static func resumeAfterRecording() {
        guard wasPlaying else { return }
        wasPlaying = false
        Task { _ = await runAdapter(["send", Self.commandPlay]) }
    }

    // MARK: - Adapter plumbing

    private static var adapterDirectory: URL? {
        Bundle.main.resourceURL?.appendingPathComponent("mediaremote-adapter")
    }

    private static var adapterScript: URL? {
        guard let dir = adapterDirectory else { return nil }
        let script = dir.appendingPathComponent("mediaremote-adapter.pl")
        return FileManager.default.fileExists(atPath: script.path) ? script : nil
    }

    private static func runAdapter(_ arguments: [String]) async -> String? {
        guard let script = adapterScript, let dir = adapterDirectory else { return nil }
        let framework = dir.appendingPathComponent("MediaRemoteAdapter.framework")

        return await withCheckedContinuation { continuation in
            DispatchQueue.global(qos: .userInitiated).async {
                let process = Process()
                process.executableURL = URL(fileURLWithPath: "/usr/bin/perl")
                process.arguments = [script.path, framework.path] + arguments
                let pipe = Pipe()
                process.standardOutput = pipe
                process.standardError = Pipe()

                // Hard kill if the adapter ever hangs — media control must
                // never stall the recording flow.
                let watchdog = DispatchWorkItem { process.terminate() }
                DispatchQueue.global().asyncAfter(deadline: .now() + 3, execute: watchdog)

                do {
                    try process.run()
                    process.waitUntilExit()
                    watchdog.cancel()
                    guard process.terminationStatus == 0 else {
                        continuation.resume(returning: nil)
                        return
                    }
                    let data = pipe.fileHandleForReading.readDataToEndOfFile()
                    continuation.resume(
                        returning: String(decoding: data, as: UTF8.self)
                            .trimmingCharacters(in: .whitespacesAndNewlines))
                } catch {
                    watchdog.cancel()
                    continuation.resume(returning: nil)
                }
            }
        }
    }
}
