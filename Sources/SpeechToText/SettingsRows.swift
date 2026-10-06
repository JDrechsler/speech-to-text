import SwiftUI

struct PrivacyBadge: View {
    let engine: EnginePreference
    var text: String?

    var body: some View {
        HStack(spacing: 4) {
            Image(systemName: engine.isLocal ? "lock.fill" : "icloud.and.arrow.up.fill")
            Text(text ?? engine.privacyBadge)
        }
        .font(.caption.weight(.semibold))
        .padding(.horizontal, 7)
        .padding(.vertical, 2)
        .background(Capsule().fill(tint.opacity(0.18)))
        .foregroundStyle(tint)
        .fixedSize()
    }

    private var tint: Color { engine.isLocal ? .green : .orange }
}

struct EngineStatus {
    let text: String
    let isReady: Bool
}

struct EngineRow<Details: View>: View {
    let engine: EnginePreference
    let isSelected: Bool
    let status: EngineStatus
    @Binding var isExpanded: Bool
    let select: () -> Void
    @ViewBuilder let details: () -> Details

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 8) {
                Button(action: select) {
                    Image(systemName: isSelected ? "largecircle.fill.circle" : "circle")
                        .font(.title3)
                        .foregroundStyle(isSelected ? Color.accentColor : .secondary)
                }
                .buttonStyle(.plain)
                .help(isSelected ? "In use" : "Use \(engine.displayName)")
                .accessibilityLabel("Use \(engine.displayName). \(engine.privacyStatement)")
                .accessibilityAddTraits(isSelected ? .isSelected : [])

                Button {
                    withAnimation(.snappy(duration: 0.2)) { isExpanded.toggle() }
                } label: {
                    HStack(spacing: 8) {
                        Text(engine.displayName).fontWeight(.semibold)
                        if !engine.isLocal {
                            Text(engine.provider).foregroundStyle(.secondary)
                        }
                        PrivacyBadge(engine: engine, text: engine.shortPrivacyBadge)
                        Spacer(minLength: 8)
                        statusLabel
                        Image(systemName: "chevron.right")
                            .font(.caption.weight(.semibold))
                            .foregroundStyle(.tertiary)
                            .rotationEffect(.degrees(isExpanded ? 90 : 0))
                    }
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .help(isExpanded ? "Hide details" : "Show details")
            }
            if isExpanded {
                details()
                    .padding(.leading, 30)
                    .padding(.bottom, 4)
            }
        }
    }

    private var statusLabel: some View {
        HStack(spacing: 4) {
            if status.isReady {
                Image(systemName: "checkmark.circle.fill").foregroundStyle(.green)
            }
            Text(status.text)
        }
        .font(.callout)
        .foregroundStyle(status.isReady || !isSelected ? .secondary : Color.orange)
        .lineLimit(1)
    }
}

struct WhisperModelRow: View {
    let model: WhisperModel
    @ObservedObject var store: WhisperModelStore
    let isEngineSelected: Bool
    let use: () -> Void
    let requestDelete: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            HStack(spacing: 8) {
                Text(model.name)
                Text(model.sizeText).foregroundStyle(.secondary)
                if model.isRecommended {
                    Text("Recommended")
                        .font(.caption2.weight(.semibold))
                        .padding(.horizontal, 6)
                        .padding(.vertical, 1)
                        .background(Capsule().fill(Color.accentColor.opacity(0.18)))
                        .foregroundStyle(Color.accentColor)
                }
                Spacer(minLength: 8)
                controls
            }
            if let failure = store.failures[model.id] {
                Text(failure).font(.caption).foregroundStyle(.red)
            }
        }
        .controlSize(.small)
        .help(model.detail)
    }

    @ViewBuilder private var controls: some View {
        if let progress = store.progress[model.id] {
            ProgressView(value: progress).frame(width: 90)
            Text("\(Int(progress * 100))%")
                .font(.caption.monospacedDigit())
                .frame(width: 32, alignment: .trailing)
            Button("Cancel") { store.cancel(model) }
        } else if store.installedIDs.contains(model.id) {
            if isEngineSelected && store.selectedID == model.id {
                Label("In use", systemImage: "checkmark")
                    .font(.callout)
                    .foregroundStyle(.green)
            } else {
                Button("Use", action: use)
            }
            Button(action: requestDelete) {
                Image(systemName: "trash")
            }
            .buttonStyle(.borderless)
            .help("Delete this model from your Mac")
        } else {
            Button("Download") { store.download(model) }
        }
    }
}

struct CredentialRow: View {
    let field: CredentialField
    @ObservedObject var credentials: CloudCredentials
    @State private var draft = ""
    @State private var isEditing = false
    @State private var saveFailed = false

    var body: some View {
        HStack(spacing: 8) {
            Text(field.label)
                .frame(width: 60, alignment: .leading)
                .foregroundStyle(.secondary)
            if credentials.isSet(field) && !isEditing {
                savedValue
                Spacer()
                Button("Change") { startEditing() }
                Button("Remove", role: .destructive) { credentials.remove(field) }
            } else {
                input
                Button("Save", action: save)
                    .disabled(draft.trimmingCharacters(in: .whitespaces).isEmpty)
                if isEditing {
                    Button("Cancel") { stopEditing() }
                }
            }
        }
        .controlSize(.small)
        .alert("Could not save to the Keychain", isPresented: $saveFailed) {
            Button("OK", role: .cancel) {}
        }
    }

    @ViewBuilder private var savedValue: some View {
        if field.isSecret {
            Label("Saved in your Keychain", systemImage: "key.fill")
                .foregroundStyle(.secondary)
        } else {
            Text(CloudCredentials.publicValue(field) ?? "")
                .lineLimit(1)
                .truncationMode(.middle)
                .textSelection(.enabled)
        }
    }

    private var input: some View {
        Group {
            if field.isSecret {
                SecureField(field.label, text: $draft, prompt: Text(field.placeholder))
            } else {
                TextField(field.label, text: $draft, prompt: Text(field.placeholder))
            }
        }
        .labelsHidden()
        .textFieldStyle(.roundedBorder)
        .onSubmit(save)
    }

    private func startEditing() {
        draft = CloudCredentials.publicValue(field) ?? ""
        isEditing = true
    }

    private func stopEditing() {
        draft = ""
        isEditing = false
    }

    private func save() {
        guard !draft.trimmingCharacters(in: .whitespaces).isEmpty else { return }
        if credentials.save(draft, for: field) {
            stopEditing()
        } else {
            saveFailed = true
        }
    }
}
