import AudioToolbox
import CoreAudio
import Foundation

struct AudioInputDevice: Equatable {
    let id: AudioDeviceID
    let uid: String
    let name: String
    let isBuiltIn: Bool
}

enum MicrophonePreference: Equatable {
    case systemDefault
    case builtIn
    case device(uid: String)

    var storageValue: String {
        switch self {
        case .systemDefault: return "default"
        case .builtIn: return "builtin"
        case .device(let uid): return "uid:\(uid)"
        }
    }

    init(storageValue: String) {
        if storageValue == "default" {
            self = .systemDefault
        } else if storageValue.hasPrefix("uid:") {
            self = .device(uid: String(storageValue.dropFirst(4)))
        } else {
            self = .builtIn
        }
    }

    func matches(_ device: AudioInputDevice) -> Bool {
        switch self {
        case .systemDefault: return false
        case .builtIn: return device.isBuiltIn
        case .device(let uid): return device.uid == uid
        }
    }
}

enum AudioDevices {
    static func inputDevices() -> [AudioInputDevice] {
        var address = propertyAddress(kAudioHardwarePropertyDevices)
        var size: UInt32 = 0
        let system = AudioObjectID(kAudioObjectSystemObject)
        guard
            AudioObjectGetPropertyDataSize(system, &address, 0, nil, &size) == noErr,
            size > 0
        else { return [] }

        var ids = [AudioDeviceID](
            repeating: 0, count: Int(size) / MemoryLayout<AudioDeviceID>.size)
        guard AudioObjectGetPropertyData(system, &address, 0, nil, &size, &ids) == noErr
        else { return [] }

        return ids.compactMap(describe)
    }

    static func defaultInputDevice() -> AudioInputDevice? {
        var address = propertyAddress(kAudioHardwarePropertyDefaultInputDevice)
        var id = AudioDeviceID(0)
        var size = UInt32(MemoryLayout<AudioDeviceID>.size)
        guard
            AudioObjectGetPropertyData(
                AudioObjectID(kAudioObjectSystemObject), &address, 0, nil, &size, &id) == noErr,
            id != AudioDeviceID(kAudioObjectUnknown)
        else { return nil }
        return describe(id)
    }

    static func resolve(_ preference: MicrophonePreference) -> AudioInputDevice? {
        switch preference {
        case .systemDefault: return defaultInputDevice()
        case .builtIn: return inputDevices().first { $0.isBuiltIn }
        case .device(let uid): return inputDevices().first { $0.uid == uid }
        }
    }

    private static func describe(_ id: AudioDeviceID) -> AudioInputDevice? {
        guard
            inputChannelCount(id) > 0,
            !isPrivateAggregate(id),
            let uid = string(id, kAudioDevicePropertyDeviceUID)
        else { return nil }
        return AudioInputDevice(
            id: id,
            uid: uid,
            name: string(id, kAudioObjectPropertyName) ?? uid,
            isBuiltIn: transportType(id) == kAudioDeviceTransportTypeBuiltIn)
    }

    /// CoreAudio spins up a per-process "CADefaultDeviceAggregate" while an
    /// AVAudioEngine exists; it is a real input device but not one anybody
    /// chose, so it must never appear in the microphone list.
    private static func isPrivateAggregate(_ id: AudioDeviceID) -> Bool {
        var address = propertyAddress(kAudioAggregateDevicePropertyComposition)
        var size = UInt32(MemoryLayout<CFDictionary?>.size)
        var value: CFDictionary?
        let status = withUnsafeMutablePointer(to: &value) {
            AudioObjectGetPropertyData(id, &address, 0, nil, &size, $0)
        }
        guard status == noErr, let composition = value as? [String: Any] else { return false }
        return (composition[kAudioAggregateDeviceIsPrivateKey] as? Int) == 1
    }

    private static func inputChannelCount(_ id: AudioDeviceID) -> Int {
        var address = propertyAddress(
            kAudioDevicePropertyStreamConfiguration, scope: kAudioObjectPropertyScopeInput)
        var size: UInt32 = 0
        guard AudioObjectGetPropertyDataSize(id, &address, 0, nil, &size) == noErr, size > 0
        else { return 0 }

        let storage = UnsafeMutableRawPointer.allocate(
            byteCount: Int(size), alignment: MemoryLayout<AudioBufferList>.alignment)
        defer { storage.deallocate() }
        guard AudioObjectGetPropertyData(id, &address, 0, nil, &size, storage) == noErr
        else { return 0 }

        let buffers = UnsafeMutableAudioBufferListPointer(
            storage.assumingMemoryBound(to: AudioBufferList.self))
        return buffers.reduce(0) { $0 + Int($1.mNumberChannels) }
    }

    private static func transportType(_ id: AudioDeviceID) -> UInt32 {
        var address = propertyAddress(kAudioDevicePropertyTransportType)
        var value: UInt32 = 0
        var size = UInt32(MemoryLayout<UInt32>.size)
        guard AudioObjectGetPropertyData(id, &address, 0, nil, &size, &value) == noErr
        else { return 0 }
        return value
    }

    private static func string(
        _ id: AudioObjectID, _ selector: AudioObjectPropertySelector
    ) -> String? {
        var address = propertyAddress(selector)
        var size = UInt32(MemoryLayout<CFString?>.size)
        var value: CFString?
        let status = withUnsafeMutablePointer(to: &value) {
            AudioObjectGetPropertyData(id, &address, 0, nil, &size, $0)
        }
        guard status == noErr, let value else { return nil }
        return value as String
    }

    private static func propertyAddress(
        _ selector: AudioObjectPropertySelector,
        scope: AudioObjectPropertyScope = kAudioObjectPropertyScopeGlobal
    ) -> AudioObjectPropertyAddress {
        AudioObjectPropertyAddress(
            mSelector: selector, mScope: scope, mElement: kAudioObjectPropertyElementMain)
    }
}
