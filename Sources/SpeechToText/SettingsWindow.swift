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
            window.setContentSize(NSSize(width: 620, height: 540))
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
    @State private var expanded: Set<EnginePreference> = []
    @State private var modelPendingDeletion: WhisperModel?
    @State private var dictionaryTermCount = PersonalDictionary.terms().count
    @State private var accessibilityAllowed = PasteService.isAccessibilityTrusted

    var body: some View {
        Form {
            if !state.enginePreference.isReady {
                Section { setupBanner }
            }
            Section {
                engineRow(.whisper) { whisperDetails }
            } header: {
                groupHeader(
                    "On this Mac", systemImage: "lock.shield.fill", tint: .green,
                    subtitle: "Private and offline. Your voice never leaves this Mac.")
            }
            Section {
                ForEach(EnginePreference.cloud) { engine in
                    engineRow(engine) { cloudDetails(engine) }
                }
            } header: {
                groupHeader(
                    "Cloud", systemImage: "icloud.and.arrow.up.fill", tint: .orange,
                    subtitle: "Each recording is uploaded to the provider. Needs your own API key.")
            }
            generalSection
        }
        .formStyle(.grouped)
        .frame(minWidth: 560, minHeight: 420)
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

    private var setupBanner: some View {
        Label {
            Text(
                state.enginePreference.isLocal
                    ? "Download a Whisper model to start dictating."
                    : "Add your \(state.enginePreference.provider) details, or pick another engine."
            )
        } icon: {
            Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(.orange)
        }
        .font(.callout.weight(.medium))
    }

    private func groupHeader(_ title: String, systemImage: String, tint: Color, subtitle: String)
        -> some View
    {
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            Label(title, systemImage: systemImage)
                .font(.headline)
                .foregroundStyle(tint)
            Text(subtitle)
                .font(.callout)
                .foregroundStyle(.secondary)
        }
        .textCase(nil)
    }

    private func engineRow<Details: View>(
        _ engine: EnginePreference, @ViewBuilder details: @escaping () -> Details
    ) -> some View {
        EngineRow(
            engine: engine,
            isSelected: state.enginePreference == engine,
            status: status(of: engine),
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
