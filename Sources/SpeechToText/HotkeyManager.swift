import Carbon.HIToolbox
import Foundation

/// Registers global hotkeys via the Carbon hotkey API (no Accessibility
/// permission required, unlike CGEvent taps).
///
/// ⌥Space — toggle recording, ⌥Esc — cancel the current recording.
final class HotkeyManager {
    var onToggle: (() -> Void)?
    var onCancel: (() -> Void)?

    private var handlerRef: EventHandlerRef?
    private var toggleRef: EventHotKeyRef?
    private var cancelRef: EventHotKeyRef?

    private static let signature: OSType = 0x5354_5854  // "STXT"
    private static let toggleID: UInt32 = 1
    private static let cancelID: UInt32 = 2

    func register() {
        var eventType = EventTypeSpec(
            eventClass: OSType(kEventClassKeyboard),
            eventKind: UInt32(kEventHotKeyPressed))

        InstallEventHandler(
            GetEventDispatcherTarget(),
            { _, event, userData -> OSStatus in
                guard let event, let userData else { return noErr }
                var hotKeyID = EventHotKeyID()
                GetEventParameter(
                    event,
                    EventParamName(kEventParamDirectObject),
                    EventParamType(typeEventHotKeyID),
                    nil,
                    MemoryLayout<EventHotKeyID>.size,
                    nil,
                    &hotKeyID)
                let manager = Unmanaged<HotkeyManager>.fromOpaque(userData)
                    .takeUnretainedValue()
                switch hotKeyID.id {
                case HotkeyManager.toggleID:
                    manager.onToggle?()
                case HotkeyManager.cancelID:
                    manager.onCancel?()
                default:
                    break
                }
                return noErr
            },
            1,
            &eventType,
            Unmanaged.passUnretained(self).toOpaque(),
            &handlerRef)

        RegisterEventHotKey(
            UInt32(kVK_Space),
            UInt32(optionKey),
            EventHotKeyID(signature: Self.signature, id: Self.toggleID),
            GetEventDispatcherTarget(),
            0,
            &toggleRef)

        RegisterEventHotKey(
            UInt32(kVK_Escape),
            UInt32(optionKey),
            EventHotKeyID(signature: Self.signature, id: Self.cancelID),
            GetEventDispatcherTarget(),
            0,
            &cancelRef)
    }

    deinit {
        if let toggleRef { UnregisterEventHotKey(toggleRef) }
        if let cancelRef { UnregisterEventHotKey(cancelRef) }
        if let handlerRef { RemoveEventHandler(handlerRef) }
    }
}
