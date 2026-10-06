import AVFoundation
import AppKit
import SwiftUI

struct HistoryView: View {
    @ObservedObject var store: HistoryStore
    let retranscriber: AppState

    @StateObject private var player = AudioPlayer()
    @State private var busyEntryID: String?
    @State private var errorMessage: String?

    var body: some View {
        Group {
            if store.entries.isEmpty {
                VStack(spacing: 8) {
                    Image(systemName: "waveform")
                        .font(.largeTitle)
                        .foregroundStyle(.secondary)
                    Text("No recordings yet")
                        .foregroundStyle(.secondary)
                    Text("Press ⌥Space anywhere to start dictating.")
                        .font(.caption)
                        .foregroundStyle(.tertiary)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                List(store.entries) { entry in
                    HistoryRow(
                        entry: entry,
                        isPlaying: player.playingID == entry.id,
                        isBusy: busyEntryID == entry.id,
                        onPlay: { togglePlayback(entry) },
                        onCopy: { PasteService.copyToClipboard(entry.transcript) },
                        onRetranscribe: { retranscribe(entry, engine: $0) },
                        defaultEngine: EnginePreference.from(historyLabel: entry.engine)
                            ?? retranscriber.enginePreference,
                        onReveal: { reveal(entry) },
                        onDelete: { store.delete(entry) })
                }
                .listStyle(.inset)
            }
        }
        .alert(
            "Re-transcription failed", isPresented: .init(
                get: { errorMessage != nil },
                set: { if !$0 { errorMessage = nil } })
        ) {
            Button("OK", role: .cancel) {}
        } message: {
            Text(errorMessage ?? "")
        }
        .frame(minWidth: 640, minHeight: 360)
    }

    private func togglePlayback(_ entry: RecordingEntry) {
        if player.playingID == entry.id {
            player.stop()
        } else {
            let url = HistoryStore.recordingsDirectory.appendingPathComponent(entry.fileName)
            player.play(url: url, id: entry.id)
        }
    }

    private func retranscribe(_ entry: RecordingEntry, engine: EnginePreference) {
        busyEntryID = entry.id
        Task {
            do {
                try await retranscriber.retranscribe(entry: entry, engine: engine)
            } catch {
                errorMessage = error.localizedDescription
            }
            busyEntryID = nil
        }
    }

    private func reveal(_ entry: RecordingEntry) {
        let url = HistoryStore.recordingsDirectory.appendingPathComponent(entry.fileName)
        NSWorkspace.shared.activateFileViewerSelecting([url])
    }
}

private struct HistoryRow: View {
    let entry: RecordingEntry
    let isPlaying: Bool
    let isBusy: Bool
    let onPlay: () -> Void
    let onCopy: () -> Void
    let onRetranscribe: (EnginePreference) -> Void
    let defaultEngine: EnginePreference
    let onReveal: () -> Void
    let onDelete: () -> Void

    private static let dateFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.dateStyle = .medium
        formatter.timeStyle = .short
        return formatter
    }()

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 12) {
                Button(action: onPlay) {
                    Image(systemName: isPlaying ? "stop.circle.fill" : "play.circle")
                        .font(.title3)
                }
                .buttonStyle(.plain)
                .help(isPlaying ? "Stop" : "Play recording")

                Text(Self.dateFormatter.string(from: entry.createdAt))
                    .font(.callout.weight(.medium))

                if entry.duration > 0 {
                    Text(durationText)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }

                Text(entry.engine)
                    .font(.caption2)
                    .padding(.horizontal, 6)
                    .padding(.vertical, 2)
                    .background(Capsule().fill(Color.secondary.opacity(0.15)))
                    .foregroundStyle(.secondary)

                Spacer()

                if isBusy {
                    ProgressView().controlSize(.small)
                } else {
                    Menu {
                        Section("On this Mac") {
                            ForEach(EnginePreference.local) { engine in
                                Button(engine.menuTitle) { onRetranscribe(engine) }
                                    .disabled(!engine.isReady)
                            }
                        }
                        Section("Cloud · uploads this recording") {
                            ForEach(EnginePreference.cloud) { engine in
                                Button(engine.menuTitle) { onRetranscribe(engine) }
                                    .disabled(!engine.isReady)
                            }
                        }
                    } label: {
                        Text("Re-transcribe")
                    } primaryAction: {
                        onRetranscribe(defaultEngine)
                    }
                    .menuStyle(.button)
                    .fixedSize()
                    .help("Click: run again with \(defaultEngine.menuTitle). Arrow: pick another engine.")

                    Button(action: onCopy) {
                        Image(systemName: "doc.on.doc")
                    }
                    .buttonStyle(.plain)
                    .help("Copy transcript")

                    Button(action: onReveal) {
                        Image(systemName: "folder")
                    }
                    .buttonStyle(.plain)
                    .help("Reveal audio file in Finder")

                    Button(role: .destructive, action: onDelete) {
                        Image(systemName: "trash")
                    }
                    .buttonStyle(.plain)
                    .help("Move audio to Trash")
                }
            }

            if entry.transcript.isEmpty {
                Text("No transcript — use Re-transcribe to recover the text.")
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .italic()
            } else {
                Text(entry.transcript)
                    .font(.callout)
                    .textSelection(.enabled)
                    .lineLimit(4)
            }
        }
        .padding(.vertical, 6)
    }

    private var durationText: String {
        let seconds = Int(entry.duration.rounded())
        return String(format: "%d:%02d", seconds / 60, seconds % 60)
    }
}

@MainActor
private final class AudioPlayer: NSObject, ObservableObject, AVAudioPlayerDelegate {
    @Published var playingID: String?
    private var player: AVAudioPlayer?

    func play(url: URL, id: String) {
        stop()
        guard let newPlayer = try? AVAudioPlayer(contentsOf: url) else { return }
        newPlayer.delegate = self
        newPlayer.play()
        player = newPlayer
        playingID = id
    }

    func stop() {
        player?.stop()
        player = nil
        playingID = nil
    }

    nonisolated func audioPlayerDidFinishPlaying(_ player: AVAudioPlayer, successfully flag: Bool)
    {
        Task { @MainActor in self.stop() }
    }
}
