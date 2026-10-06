import AppKit
import ServiceManagement
import SwiftUI

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    private var statusItem: NSStatusItem!
    private let hotkeys = HotkeyManager()
    private var historyWindow: NSWindow?
    private var cleanupTimer: Timer?

    let state = AppState()
    private lazy var settings = SettingsWindowController(state: state)

    func applicationDidFinishLaunching(_ notification: Notification) {
        setUpStatusItem()

        hotkeys.onToggle = { [weak self] in self?.state.toggle() }
        hotkeys.onCancel = { [weak self] in self?.state.cancelRecording() }
        hotkeys.register()

        PersonalDictionary.ensureFileExists()
        state.onPhaseChange = { [weak self] in self?.refreshStatusItem() }
        state.onNeedsSetup = { [weak self] in self?.settings.show() }
        state.prepareEngine()
        refreshStatusItem()

        if !state.enginePreference.isReady {
            settings.show()
        }

        // Retention: recordings are kept for 24h (cleanup also runs at launch,
        // inside HistoryStore.init).
        cleanupTimer = Timer.scheduledTimer(withTimeInterval: 3600, repeats: true) {
            [weak self] _ in
            Task { @MainActor in self?.state.historyStore.cleanupExpired() }
        }
    }

    func applicationWillTerminate(_ notification: Notification) {
        LocalWhisper.shared.releaseBeforeExit()
    }

    // MARK: - Status item

    private func setUpStatusItem() {
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        statusItem.menu = buildMenu()
        refreshStatusItem()
    }

    private func buildMenu() -> NSMenu {
        let menu = NSMenu()

        let toggleItem = NSMenuItem(
            title: "Start Recording", action: #selector(toggleRecording), keyEquivalent: "")
        toggleItem.target = self
        toggleItem.tag = MenuTag.toggle.rawValue
        menu.addItem(toggleItem)

        menu.addItem(NSMenuItem.separator())

        let historyItem = NSMenuItem(
            title: "History…", action: #selector(showHistory), keyEquivalent: "h")
        historyItem.target = self
        menu.addItem(historyItem)

        let folderItem = NSMenuItem(
            title: "Open Recordings Folder", action: #selector(openRecordingsFolder),
            keyEquivalent: "")
        folderItem.target = self
        menu.addItem(folderItem)

        let dictionaryItem = NSMenuItem(
            title: "Edit Dictionary…", action: #selector(openDictionary),
            keyEquivalent: "d")
        dictionaryItem.target = self
        menu.addItem(dictionaryItem)

        menu.addItem(NSMenuItem.separator())

        let engineItem = NSMenuItem(title: "Engine", action: nil, keyEquivalent: "")
        engineItem.tag = MenuTag.engine.rawValue
        engineItem.submenu = NSMenu()
        menu.addItem(engineItem)

        let microphoneItem = NSMenuItem(title: "Microphone", action: nil, keyEquivalent: "")
        microphoneItem.tag = MenuTag.microphone.rawValue
        microphoneItem.submenu = NSMenu()
        menu.addItem(microphoneItem)

        let pauseMediaItem = NSMenuItem(
            title: "Pause Media While Recording",
            action: #selector(togglePauseMedia), keyEquivalent: "")
        pauseMediaItem.target = self
        pauseMediaItem.tag = MenuTag.pauseMedia.rawValue
        menu.addItem(pauseMediaItem)

        let axItem = NSMenuItem(
            title: "Grant Accessibility (needed for auto-paste)…",
            action: #selector(promptAccessibility), keyEquivalent: "")
        axItem.target = self
        axItem.tag = MenuTag.accessibility.rawValue
        menu.addItem(axItem)

        let loginItem = NSMenuItem(
            title: "Launch at Login", action: #selector(toggleLaunchAtLogin),
            keyEquivalent: "")
        loginItem.target = self
        loginItem.tag = MenuTag.launchAtLogin.rawValue
        menu.addItem(loginItem)

        menu.addItem(NSMenuItem.separator())

        let settingsItem = NSMenuItem(
            title: "Settings…", action: #selector(showSettings), keyEquivalent: ",")
        settingsItem.target = self
        menu.addItem(settingsItem)

        menu.addItem(NSMenuItem.separator())

        let quitItem = NSMenuItem(
            title: "Quit Speech to Text", action: #selector(NSApplication.terminate(_:)),
            keyEquivalent: "q")
        menu.addItem(quitItem)

        menu.delegate = self
        return menu
    }

    private enum MenuTag: Int {
        case toggle = 1
        case accessibility = 2
        case pauseMedia = 3
        case engine = 4
        case launchAtLogin = 6
        case microphone = 7
    }

    func refreshStatusItem() {
        guard let button = statusItem?.button else { return }
        let (symbol, description): (String, String)
        switch state.phase {
        case .idle:
            (symbol, description) = ("mic", "Idle")
        case .recording:
            (symbol, description) = ("record.circle.fill", "Recording")
        case .finishing:
            (symbol, description) = ("ellipsis.circle", "Finishing")
        }
        button.image = NSImage(
            systemSymbolName: symbol, accessibilityDescription: description)
        button.contentTintColor = state.phase == .recording ? .systemRed : nil
    }

    // MARK: - Actions

    @objc private func toggleRecording() {
        state.toggle()
    }

    @objc private func showHistory() {
        if historyWindow == nil {
            let view = HistoryView(store: state.historyStore, retranscriber: state)
            let hosting = NSHostingController(rootView: view)
            let window = NSWindow(contentViewController: hosting)
            window.title = "Recording History"
            window.setContentSize(NSSize(width: 720, height: 480))
            window.styleMask = [.titled, .closable, .resizable, .miniaturizable]
            window.isReleasedWhenClosed = false
            window.center()
            historyWindow = window
        }
        state.historyStore.reload()
        historyWindow?.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
    }

    @objc private func openRecordingsFolder() {
        NSWorkspace.shared.open(HistoryStore.recordingsDirectory)
    }

    @objc private func openDictionary() {
        PersonalDictionary.ensureFileExists()
        NSWorkspace.shared.open(PersonalDictionary.fileURL)
    }

    @objc private func showSettings() {
        settings.show()
    }

    @objc private func promptAccessibility() {
        PasteService.ensureAccessibility(prompt: true)
    }

    @objc private func togglePauseMedia() {
        state.pauseMediaEnabled.toggle()
    }

    @objc private func toggleLaunchAtLogin() {
        let service = SMAppService.mainApp
        do {
            if service.status == .enabled {
                try service.unregister()
            } else {
                try service.register()
            }
        } catch {
            NSLog("Launch at login toggle failed: \(error)")
        }
    }

    @objc private func selectMicrophone(_ sender: NSMenuItem) {
        guard let stored = sender.representedObject as? String else { return }
        state.microphonePreference = MicrophonePreference(storageValue: stored)
    }

    @objc private func selectEngine(_ sender: NSMenuItem) {
        guard let raw = sender.representedObject as? String,
            let preference = EnginePreference(rawValue: raw)
        else { return }
        state.enginePreference = preference
        if !preference.isReady {
            settings.show()
        }
    }
}

extension AppDelegate {
    fileprivate func rebuildEngineMenu(_ item: NSMenuItem) {
        let submenu = NSMenu()
        let groups: [(String, [EnginePreference])] = [
            ("On this Mac · private, offline", EnginePreference.local),
            ("Cloud · uploads your audio", EnginePreference.cloud),
        ]
        for (header, engines) in groups {
            submenu.addItem(NSMenuItem.sectionHeader(title: header))
            for engine in engines {
                let title = engine.isReady ? engine.menuTitle : "\(engine.menuTitle) (set up…)"
                let engineItem = NSMenuItem(
                    title: title, action: #selector(selectEngine(_:)), keyEquivalent: "")
                engineItem.target = self
                engineItem.representedObject = engine.rawValue
                engineItem.state = engine == state.enginePreference ? .on : .off
                submenu.addItem(engineItem)
            }
        }
        item.submenu = submenu
        let current = state.enginePreference
        item.title = "Engine: \(current.displayName) (\(current.isLocal ? "on this Mac" : "cloud"))"
    }

    fileprivate func rebuildMicrophoneMenu(_ item: NSMenuItem) {
        let devices = AudioDevices.inputDevices()
        let preference = state.microphonePreference
        let inUse = AudioDevices.resolve(preference)

        let submenu = NSMenu()

        let systemDefault = AudioDevices.defaultInputDevice()
        let defaultItem = NSMenuItem(
            title: "System Default"
                + (systemDefault.map { " (\($0.name))" } ?? ""),
            action: #selector(selectMicrophone(_:)), keyEquivalent: "")
        defaultItem.target = self
        defaultItem.representedObject = MicrophonePreference.systemDefault.storageValue
        defaultItem.state = preference == .systemDefault ? .on : .off
        submenu.addItem(defaultItem)

        submenu.addItem(NSMenuItem.separator())

        for device in devices {
            let title = device.isBuiltIn ? "\(device.name) (preferred)" : device.name
            let deviceItem = NSMenuItem(
                title: title, action: #selector(selectMicrophone(_:)), keyEquivalent: "")
            deviceItem.target = self
            deviceItem.representedObject =
                device.isBuiltIn
                ? MicrophonePreference.builtIn.storageValue
                : MicrophonePreference.device(uid: device.uid).storageValue
            deviceItem.state = preference.matches(device) ? .on : .off
            submenu.addItem(deviceItem)
        }

        if preference != .systemDefault, inUse == nil {
            submenu.addItem(NSMenuItem.separator())
            let missing = NSMenuItem(
                title: "Preferred microphone not connected, using system default",
                action: nil, keyEquivalent: "")
            missing.isEnabled = false
            submenu.addItem(missing)
        }

        item.submenu = submenu
        item.title = "Microphone: \(inUse?.name ?? systemDefault?.name ?? "none")"
    }
}

extension AppDelegate: NSMenuDelegate {
    func menuNeedsUpdate(_ menu: NSMenu) {
        if let toggleItem = menu.item(withTag: MenuTag.toggle.rawValue) {
            switch state.phase {
            case .recording:
                toggleItem.title = "Stop Recording (⌥Space)"
            case .finishing:
                toggleItem.title = "Finishing…"
            case .idle:
                toggleItem.title = "Start Recording (⌥Space)"
            }
        }
        if let axItem = menu.item(withTag: MenuTag.accessibility.rawValue) {
            axItem.isHidden = PasteService.isAccessibilityTrusted
        }
        if let pauseMediaItem = menu.item(withTag: MenuTag.pauseMedia.rawValue) {
            pauseMediaItem.state = state.pauseMediaEnabled ? .on : .off
        }
        if let loginItem = menu.item(withTag: MenuTag.launchAtLogin.rawValue) {
            loginItem.state = SMAppService.mainApp.status == .enabled ? .on : .off
        }
        if let microphoneItem = menu.item(withTag: MenuTag.microphone.rawValue) {
            rebuildMicrophoneMenu(microphoneItem)
        }
        if let engineItem = menu.item(withTag: MenuTag.engine.rawValue) {
            rebuildEngineMenu(engineItem)
        }
    }
}
