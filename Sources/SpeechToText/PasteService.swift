import AppKit
import ApplicationServices

/// Clipboard + synthetic ⌘V paste into whatever app currently has focus.
///
/// The overlay panel is non-activating, so keyboard focus never leaves the
/// app the user was typing in; posting ⌘V lands the text right there.
enum PasteService {
    static var isAccessibilityTrusted: Bool {
        AXIsProcessTrusted()
    }

    @discardableResult
    static func ensureAccessibility(prompt: Bool) -> Bool {
        let options =
            [kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String: prompt]
            as CFDictionary
        return AXIsProcessTrustedWithOptions(options)
    }

    static func copyToClipboard(_ text: String) {
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        pasteboard.setString(text, forType: .string)
    }

    /// Synthesizes ⌘V. The text must already be on the clipboard.
    /// No-op (clipboard-only) when Accessibility permission is missing —
    /// the first attempt triggers the system permission prompt.
    static func pasteIntoFocusedField() {
        guard ensureAccessibility(prompt: true) else { return }

        // Small delay so the pasteboard write settles before the paste lands.
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) {
            guard let source = CGEventSource(stateID: .combinedSessionState) else { return }
            let vKeyCode: CGKeyCode = 9
            let keyDown = CGEvent(
                keyboardEventSource: source, virtualKey: vKeyCode, keyDown: true)
            let keyUp = CGEvent(
                keyboardEventSource: source, virtualKey: vKeyCode, keyDown: false)
            keyDown?.flags = .maskCommand
            keyUp?.flags = .maskCommand
            keyDown?.post(tap: .cghidEventTap)
            keyUp?.post(tap: .cghidEventTap)
        }
    }
}
