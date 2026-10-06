import AppKit
import AVFoundation

enum SessionPhase {
    case idle
    case recording
    case finishing
}

@MainActor
final class AppState: ObservableObject {
    @Published private(set) var phase: SessionPhase = .idle
    @Published var levels: [Float] = []
    @Published var statusMessage: String = ""

    let historyStore = HistoryStore()
    var onPhaseChange: (() -> Void)?
    var onNeedsSetup: (() -> Void)?

    /// Pause playing media (Spotify, YouTube, …) while recording and resume
    /// it when the recording ends.
    @Published var pauseMediaEnabled: Bool =
        UserDefaults.standard.object(forKey: "pauseMediaEnabled") as? Bool ?? true
    {
        didSet { UserDefaults.standard.set(pauseMediaEnabled, forKey: "pauseMediaEnabled") }
    }

    @Published var microphonePreference: MicrophonePreference = MicrophonePreference(
        storageValue: UserDefaults.standard.string(forKey: "microphonePreference")
            ?? "builtin")
    {
        didSet {
            UserDefaults.standard.set(
                microphonePreference.storageValue, forKey: "microphonePreference")
        }
    }

    @Published var enginePreference: EnginePreference = .stored {
        didSet {
            UserDefaults.standard.set(enginePreference.rawValue, forKey: EnginePreference.storageKey)
            if !enginePreference.isLocal {
                UserDefaults.standard.set(
                    enginePreference.rawValue, forKey: EnginePreference.lastCloudStorageKey)
            }
            statusMessage = ""
            prepareEngine()
        }
    }

    private var recorder: AudioRecorder?
    private var overlay: OverlayController?
    private var currentRecordingURL: URL?

    // Must cover the widest waveform the overlay can show (~60 bars at full
    // width); the view trims to what fits, so oversizing is harmless.
    private static let maxLevelBars = 96

    /// Levels arrive in bursts — one device buffer carries several 50 ms
    /// windows — so they are queued here and released a tick at a time.
    /// Appending a whole burst at once would make the wave jump instead of
    /// scroll, which is exactly the stutter this pump exists to prevent.
    private var pendingLevels: [Float] = []
    private var levelPump: Timer?
    /// Matches AudioRecorder's 50 ms level window, so on average the pump
    /// drains at precisely the rate levels are produced.
    private static let levelPumpInterval = 0.05

    // MARK: - Engine readiness

    func prepareEngine() {
        if enginePreference.isLocal {
            LocalWhisper.shared.preload(WhisperModelStore.shared.selectedModel)
        } else {
            LocalWhisper.shared.unload()
        }
    }

    private func ensureEngineReady() -> Bool {
        guard enginePreference.isReady else {
            statusMessage = enginePreference.setupHint
            onNeedsSetup?()
            return false
        }
        return true
    }

    // MARK: - Hotkey entry points

    func toggle() {
        switch phase {
        case .idle:
            guard ensureEngineReady() else { return }
            startRecording()
        case .recording:
            Task { await finishRecording() }
        case .finishing:
            break
        }
    }

    func cancelRecording() {
        guard phase == .recording else { return }
        let recorder = self.recorder
        let url = currentRecordingURL
        let duration = recorder?.duration ?? 0
        self.recorder = nil
        self.currentRecordingURL = nil

        recorder?.stop()
        stopLevelPump()
        MediaController.resumeAfterRecording()

        // Cancelled audio is still kept: save it to history without pasting.
        if let url {
            historyStore.saveEntry(
                audioURL: url, transcript: "", engine: "cancelled", duration: duration)
        }
        overlay?.hide()
        setPhase(.idle)
    }

    // MARK: - Waveform pump

    /// Releases queued levels into the displayed waveform at a steady rate so
    /// the wave scrolls smoothly regardless of the input device's buffer size.
    private func startLevelPump() {
        levelPump?.invalidate()
        let timer = Timer(timeInterval: Self.levelPumpInterval, repeats: true) { [weak self] _ in
            Task { @MainActor in self?.drainLevels() }
        }
        // .common so the wave keeps moving while a menu or resize is tracking.
        RunLoop.main.add(timer, forMode: .common)
        levelPump = timer
    }

    private func stopLevelPump() {
        levelPump?.invalidate()
        levelPump = nil
        pendingLevels = []
    }

    private func drainLevels() {
        guard phase == .recording, !pendingLevels.isEmpty else { return }
        // Producer and pump share a nominal rate, but device buffers arrive in
        // bursts; taking two when the queue backs up keeps the wave from
        // drifting behind the microphone instead of accumulating latency.
        let take = pendingLevels.count > 3 ? 2 : 1
        levels.append(contentsOf: pendingLevels.prefix(take))
        pendingLevels.removeFirst(take)
        if levels.count > Self.maxLevelBars {
            levels.removeFirst(levels.count - Self.maxLevelBars)
        }
    }

    // MARK: - Recording

    private func startRecording() {
        levels = []
        pendingLevels = []
        statusMessage = ""

        let preferred = AudioDevices.resolve(microphonePreference)
        let microphone = preferred ?? AudioDevices.defaultInputDevice()
        if preferred == nil, microphonePreference != .systemDefault {
            statusMessage =
                "Preferred microphone unavailable, using \(microphone?.name ?? "system default")"
        }

        let url = historyStore.newRecordingURL()
        currentRecordingURL = url
        let recorder = AudioRecorder(url: url, deviceUID: microphone?.uid)
        self.recorder = recorder

        recorder.onLevel = { [weak self] level in
            Task { @MainActor in
                guard let self, self.phase == .recording else { return }
                self.pendingLevels.append(level)
            }
        }
        startLevelPump()

        Task {
            guard await Self.requestMicrophoneAccess() else {
                self.statusMessage = "Microphone access denied"
                self.showMicrophoneDeniedAlert()
                self.stopLevelPump()
                self.recorder = nil
                self.currentRecordingURL = nil
                self.setPhase(.idle)
                return
            }

            // Pause whatever is playing before the mic opens; give the
            // audio a beat to fade so its tail doesn't end up recorded.
            if self.pauseMediaEnabled, await MediaController.pauseForRecording() {
                try? await Task.sleep(nanoseconds: 250_000_000)
            }

            do {
                try recorder.start()
            } catch {
                self.statusMessage = "Could not start recording: \(error.localizedDescription)"
                self.stopLevelPump()
                self.recorder = nil
                MediaController.resumeAfterRecording()
                self.setPhase(.idle)
                return
            }

            self.setPhase(.recording)
            self.showOverlay()
        }
    }

    private func finishRecording() async {
        guard phase == .recording, let recorder else { return }
        setPhase(.finishing)
        statusMessage = "Finalizing…"

        let duration = recorder.duration
        recorder.stop()
        stopLevelPump()
        self.recorder = nil
        MediaController.resumeAfterRecording()

        let url = currentRecordingURL
        currentRecordingURL = nil

        var text = ""
        let engine = enginePreference
        let engineLabel = engine.historyLabel

        var transcriptionFailed = false
        if let url {
            var attemptsMade = 0
            do {
                text = try await transcribe(
                    url, engine: engine, recordingSeconds: duration,
                    onAttempt: { attempt in
                        attemptsMade = attempt
                        self.statusMessage = attempt == 1
                            ? "Transcribing (\(engine.shortName))…"
                            : "Transcribing (\(engine.shortName))… retry \(attempt)/\(CloudRetry.attempts)"
                    })
            } catch {
                transcriptionFailed = true
                statusMessage = attemptsMade > 1
                    ? "\(error.localizedDescription) (gave up after \(attemptsMade) tries)"
                    : error.localizedDescription
                text = ""
            }
        }

        let finalText = text.trimmingCharacters(in: .whitespacesAndNewlines)

        if let url {
            historyStore.saveEntry(
                audioURL: url, transcript: finalText, engine: engineLabel, duration: duration)
        }

        if finalText.isEmpty {
            if transcriptionFailed {
                try? await Task.sleep(nanoseconds: 3_000_000_000)
            } else {
                statusMessage = "No speech detected. Audio saved to history."
                try? await Task.sleep(nanoseconds: 1_500_000_000)
            }
        } else {
            PasteService.copyToClipboard(finalText)
            PasteService.pasteIntoFocusedField()
        }

        overlay?.hide()
        statusMessage = ""
        setPhase(.idle)
    }

    // MARK: - Re-transcription (used by the history window)

    @discardableResult
    func retranscribe(entry: RecordingEntry, engine: EnginePreference) async throws -> String {
        let audioURL = HistoryStore.recordingsDirectory.appendingPathComponent(entry.fileName)
        let text = try await transcribe(
            audioURL, engine: engine, recordingSeconds: entry.duration, onAttempt: { _ in })
        let cleaned = text.trimmingCharacters(in: .whitespacesAndNewlines)
        historyStore.updateTranscript(id: entry.id, transcript: cleaned, engine: engine.historyLabel)
        PasteService.copyToClipboard(cleaned)
        return cleaned
    }

    // MARK: - Engines

    private func transcribe(
        _ url: URL, engine: EnginePreference, recordingSeconds: Double,
        onAttempt: @escaping @MainActor (Int) -> Void
    ) async throws -> String {
        let cloudCall: () async throws -> String
        switch engine {
        case .whisper:
            onAttempt(1)
            return try await LocalWhisper.shared.transcribe(
                fileURL: url, model: WhisperModelStore.shared.selectedModel,
                language: WhisperLanguage.current,
                prompt: Self.firstPersonVocabularyPrompt(PersonalDictionary.terms()))
        case .mai: cloudCall = { try await MAIClient.transcribe(fileURL: url) }
        case .scribe: cloudCall = { try await ScribeClient.transcribe(fileURL: url) }
        case .assembly: cloudCall = { try await AssemblyClient.transcribe(fileURL: url) }
        case .stepfun: cloudCall = { try await StepFunClient.transcribe(fileURL: url) }
        }
        return try await CloudRetry.run(
            engine: engine, recordingSeconds: recordingSeconds, onAttempt: onAttempt, cloudCall)
    }

    private static func firstPersonVocabularyPrompt(_ terms: [String]) -> String? {
        guard !terms.isEmpty else { return nil }
        return "Note to self. I sometimes mention " + terms.joined(separator: ", ") + "."
    }

    // MARK: - Helpers

    private func setPhase(_ newPhase: SessionPhase) {
        phase = newPhase
        onPhaseChange?()
    }

    private func showOverlay() {
        if overlay == nil {
            overlay = OverlayController(state: self)
        }
        overlay?.show()
    }

    private static func requestMicrophoneAccess() async -> Bool {
        switch AVCaptureDevice.authorizationStatus(for: .audio) {
        case .authorized:
            return true
        case .notDetermined:
            return await AVCaptureDevice.requestAccess(for: .audio)
        default:
            return false
        }
    }

    private func showMicrophoneDeniedAlert() {
        let alert = NSAlert()
        alert.messageText = "Microphone access denied"
        alert.informativeText =
            "Enable microphone access for Speech to Text in System Settings → Privacy & Security → Microphone."
        alert.addButton(withTitle: "Open System Settings")
        alert.addButton(withTitle: "Cancel")
        NSApp.activate(ignoringOtherApps: true)
        if alert.runModal() == .alertFirstButtonReturn,
            let url = URL(
                string:
                    "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone")
        {
            NSWorkspace.shared.open(url)
        }
    }
}
