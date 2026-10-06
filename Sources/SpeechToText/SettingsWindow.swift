import AppKit
import SwiftUI

@MainActor
final class SettingsWindowController {
    private var window: NSWindow?
    private let state: AppState

    init(state: AppState) {
        self.state = state
    }

    func show() {
        if window == nil {
            let hosting = NSHostingController(rootView: SettingsView(state: state))
            let window = NSWindow(contentViewController: hosting)
            window.title = "Speech to Text Settings"
            window.styleMask = [.titled, .closable, .resizable, .miniaturizable]
            window.setContentSize(NSSize(width: 600, height: 500))
            window.isReleasedWhenClosed = false
            window.center()
            self.window = window
        }
        WhisperModelStore.shared.refresh()
        CloudCredentials.shared.refresh()
        window?.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        DispatchQueue.main.async { [weak window] in window?.makeFirstResponder(nil) }
    }
}

struct SettingsView: View {
    @ObservedObject var state: AppState
    @ObservedObject private var models = WhisperModelStore.shared
    @ObservedObject private var credentials = CloudCredentials.shared
    @AppStorage(WhisperLanguage.storageKey) private var whisperLanguage = "auto"
    @AppStorage(StepFunClient.languageKey) private var stepFunLanguage = "en"
    @State private var expanded: Set<EnginePreference> = []
    @State private var modelPendingDeletion: WhisperModel?
    @State private var dictionaryTermCount = PersonalDictionary.terms().count
    @State private var accessibilityAllowed = PasteService.isAccessibilityTrusted

    var body: some View {
        Form {
            Section {
                Picker("Transcribe", selection: usesCloud) {
                    Label("On this Mac", systemImage: "lock.fill").tag(false)
                    Label("Cloud", systemImage: "icloud.fill").tag(true)
                }
                .pickerStyle(.segmented)
                .labelsHidden()
                privacyNote
                if !state.enginePreference.isReady { setupBanner }
            } header: {
                Text("Where your speech is transcribed")
            }
            Section {
                if state.enginePreference.isLocal {
                    engineRow(.whisper, showsSelector: false) { whisperDetails }
                } else {
                    ForEach(EnginePreference.cloud) { engine in
                        engineRow(engine) { cloudDetails(engine) }
                    }
                }
            }
            generalSection
        }
        .formStyle(.grouped)
        .frame(minWidth: 540, minHeight: 380)
        .onAppear(perform: expandEngineThatNeedsSetup)
        .confirmationDialog(
            "Delete \(modelPendingDeletion?.name ?? "model")?",
            isPresented: .init(
                get: { modelPendingDeletion != nil },
                set: { if !$0 { modelPendingDeletion = nil } }),
            presenting: modelPendingDeletion
        ) { model in
            Button("Delete \(model.sizeText)", role: .destructive) { models.delete(model) }
        } message: { _ in
            Text("You can download it again at any time.")
        }
        .onReceive(
            NotificationCenter.default.publisher(for: NSApplication.didBecomeActiveNotification)
        ) { _ in
            dictionaryTermCount = PersonalDictionary.terms().count
            accessibilityAllowed = PasteService.isAccessibilityTrusted
            models.refresh()
        }
    }

    private var usesCloud: Binding<Bool> {
        .init(
            get: { !state.enginePreference.isLocal },
            set: { wantsCloud in select(wantsCloud ? .preferredCloud : .whisper) })
    }

    private var privacyNote: some View {
        let isLocal = state.enginePreference.isLocal
        return Label {
            Text(
                isLocal
                    ? "Private and offline. Your voice never leaves this Mac."
                    : "Each recording is uploaded to the provider you pick. Needs your own API key."
            )
        } icon: {
            Image(systemName: isLocal ? "lock.shield.fill" : "icloud.and.arrow.up.fill")
        }
        .font(.callout)
        .foregroundStyle(isLocal ? .green : .orange)
    }

    private var setupBanner: some View {
        Label {
            Text(
                state.enginePreference.isLocal
                    ? "Download a Whisper model to start dictating."
                    : "Add your \(state.enginePreference.provider) API key below, or pick another provider."
            )
            .foregroundStyle(.primary)
        } icon: {
            Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(.orange)
        }
        .font(.callout.weight(.medium))
    }

    private func engineRow<Details: View>(
        _ engine: EnginePreference, showsSelector: Bool = true,
        @ViewBuilder details: @escaping () -> Details
    ) -> some View {
        EngineRow(
            engine: engine,
            isSelected: state.enginePreference == engine,
            status: status(of: engine),
            showsSelector: showsSelector,
            isExpanded: .init(
                get: { expanded.contains(engine) },
                set: { isOpen in
                    if isOpen { expanded.insert(engine) } else { expanded.remove(engine) }
                }),
            select: { select(engine) },
            details: details)
    }

    private func status(of engine: EnginePreference) -> EngineStatus {
        if engine.isLocal {
            let model = models.selectedModel
            if let progress = models.progress.values.max() {
                return EngineStatus(text: "Downloading \(Int(progress * 100))%", isReady: false)
            }
            return models.installedIDs.contains(model.id)
                ? EngineStatus(text: model.name, isReady: true)
                : EngineStatus(text: "No model yet", isReady: false)
        }
        return engine.isReady
            ? EngineStatus(text: "Ready", isReady: true)
            : EngineStatus(text: "Needs API key", isReady: false)
    }

    private func select(_ engine: EnginePreference) {
        state.enginePreference = engine
        if !engine.isReady {
            withAnimation(.snappy(duration: 0.2)) { _ = expanded.insert(engine) }
        }
    }

    private func expandEngineThatNeedsSetup() {
        if !state.enginePreference.isReady {
            expanded.insert(state.enginePreference)
        }
    }

    @ViewBuilder private var whisperDetails: some View {
        VStack(alignment: .leading, spacing: 8) {
            ForEach(WhisperModel.catalog) { model in
                WhisperModelRow(
                    model: model, store: models,
                    isEngineSelected: state.enginePreference == .whisper,
                    use: {
                        models.selectedID = model.id
                        state.enginePreference = .whisper
                    },
                    requestDelete: { modelPendingDeletion = model })
            }
            Text("Bigger models are more accurate and use more memory. Hover a model for details.")
                .font(.caption)
                .foregroundStyle(.secondary)
            Picker("Spoken language", selection: $whisperLanguage) {
                ForEach(WhisperLanguage.options, id: \.code) { option in
                    Text(option.name).tag(option.code)
                }
            }
            .controlSize(.small)
            .fixedSize()
        }
    }

    @ViewBuilder private func cloudDetails(_ engine: EnginePreference) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(engine.summary)
                .font(.caption)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            ForEach(engine.credentialFields) { field in
                CredentialRow(field: field, credentials: credentials)
            }
            if engine == .stepfun {
                Picker("Spoken language", selection: $stepFunLanguage) {
                    ForEach(StepFunClient.languages, id: \.code) { option in
                        Text(option.name).tag(option.code)
                    }
                }
                .controlSize(.small)
                .fixedSize()
            }
            HStack(spacing: 16) {
                if let url = engine.keyHelpURL {
                    Link("How to get a key", destination: url)
                }
                if let url = engine.privacyPolicyURL {
                    Link("\(engine.provider) privacy policy", destination: url)
                }
            }
            .font(.caption)
        }
    }

    private var generalSection: some View {
        Section {
            LabeledContent {
                Button("Edit…") {
                    PersonalDictionary.ensureFileExists()
                    NSWorkspace.shared.open(PersonalDictionary.fileURL)
                }
            } label: {
                Text("Personal dictionary")
                Text(
                    "\(dictionaryTermCount == 1 ? "1 word" : "\(dictionaryTermCount) words") the engines should spell your way. Stays on this Mac, sent along to cloud engines."
                )
            }
            LabeledContent("Shortcuts", value: "⌥ Space start and stop · ⌥ Esc cancel")
            LabeledContent("Auto-paste into the active app") {
                if accessibilityAllowed {
                    Label("Allowed", systemImage: "checkmark.circle.fill").foregroundStyle(.green)
                } else {
                    Button("Allow…") { PasteService.ensureAccessibility(prompt: true) }
                }
            }
            LabeledContent {
                Button("Open Folder") {
                    NSWorkspace.shared.open(HistoryStore.recordingsDirectory)
                }
            } label: {
                Text("Recordings")
                Text("Kept on this Mac for 24 hours, then deleted.")
            }
        } header: {
            Text("General")
        }
    }
}
