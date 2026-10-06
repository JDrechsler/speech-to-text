import AppKit
import SwiftUI

/// Floating HUD shown while recording. The panel is non-activating, so the
/// app the user was typing in keeps keyboard focus and the final paste lands
/// in the right place.
@MainActor
final class OverlayController {
    private let panel: NSPanel
    private let hosting: NSHostingView<OverlayView>
    private let state: AppState

    static let panelWidth: CGFloat = 560

    init(state: AppState) {
        self.state = state
        let size = NSSize(width: Self.panelWidth, height: OverlayView.height)
        panel = NSPanel(
            contentRect: NSRect(origin: .zero, size: size),
            styleMask: [.nonactivatingPanel, .borderless],
            backing: .buffered,
            defer: false)
        panel.level = .statusBar
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = true
        panel.hidesOnDeactivate = false
        panel.isMovableByWindowBackground = true
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]

        hosting = NSHostingView(rootView: OverlayView(state: state))
        hosting.frame = NSRect(origin: .zero, size: size)
        panel.contentView = hosting
    }

    func show() {
        position(size: panel.frame.size)
        panel.orderFrontRegardless()
    }

    func hide() {
        panel.orderOut(nil)
    }

    /// Bottom-center of the screen the user is working on.
    private func position(size: NSSize) {
        let screen =
            NSScreen.screens.first(where: { $0.frame.contains(NSEvent.mouseLocation) })
            ?? NSScreen.main
        guard let screen else { return }
        let visible = screen.visibleFrame
        let origin = NSPoint(
            x: visible.midX - size.width / 2,
            y: visible.minY + 80)
        panel.setFrameOrigin(origin)
    }
}

// MARK: - SwiftUI content

struct OverlayView: View {
    @ObservedObject var state: AppState

    var body: some View {
        VStack(spacing: 10) {
            HStack(spacing: 14) {
                Text(phaseLabel)
                    .font(.system(size: 14))
                    .foregroundStyle(.gray)

                WaveformView(levels: state.levels)
                    .frame(maxWidth: .infinity)
                    .frame(height: 48)
            }

            HStack {
                if !state.statusMessage.isEmpty {
                    Text(state.statusMessage)
                        .font(.caption)
                        .foregroundStyle(.yellow)
                } else {
                    Text("⌥Space finish & paste   ·   ⌥Esc cancel")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                Spacer()
                PrivacyBadge(engine: state.enginePreference)
                if state.phase == .recording {
                    HStack(spacing: 5) {
                        Circle().fill(.red).frame(width: 7, height: 7)
                        Text("REC")
                            .font(.caption.weight(.semibold))
                            .foregroundStyle(.red)
                    }
                }
            }
        }
        .padding(16)
        .frame(width: OverlayController.panelWidth, height: OverlayView.height, alignment: .top)
        .background(
            RoundedRectangle(cornerRadius: 18, style: .continuous)
                .fill(Color.black.opacity(0.82))
        )
        .overlay(
            RoundedRectangle(cornerRadius: 18, style: .continuous)
                .strokeBorder(Color.white.opacity(0.12), lineWidth: 1)
        )
        .environment(\.colorScheme, .dark)
    }

    private var phaseLabel: String {
        state.phase == .finishing ? "Finishing…" : "Listening…"
    }

    static let height: CGFloat = 106
}

struct WaveformView: View {
    let levels: [Float]

    private static let barWidth: CGFloat = 4
    private static let barSpacing: CGFloat = 3

    var body: some View {
        GeometryReader { geo in
            let barCount = max(1, Int((geo.size.width + Self.barSpacing) / (Self.barWidth + Self.barSpacing)))
            HStack(alignment: .center, spacing: Self.barSpacing) {
                ForEach(0..<barCount, id: \.self) { index in
                    Capsule()
                        .fill(Color.accentColor.opacity(0.9))
                        .frame(width: Self.barWidth, height: barHeight(at: index, of: barCount, maxHeight: geo.size.height))
                }
            }
            .frame(width: geo.size.width, height: geo.size.height)
        }
        // Matches AppState's level pump interval: each bar glides for exactly
        // one tick, so the wave reads as continuous motion rather than steps.
        .animation(.linear(duration: 0.05), value: levels)
    }

    private func barHeight(at index: Int, of barCount: Int, maxHeight: CGFloat) -> CGFloat {
        // Right-align the most recent levels so the wave scrolls leftward.
        let visible = levels.suffix(barCount)
        let offset = barCount - visible.count
        guard index >= offset else { return 4 }
        let level = visible[visible.startIndex + (index - offset)]
        // sqrt boosts quiet speech so the wave visibly moves at normal volume.
        return max(4, CGFloat(sqrt(level)) * maxHeight)
    }
}
