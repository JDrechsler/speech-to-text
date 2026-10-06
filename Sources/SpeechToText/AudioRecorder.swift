import AVFoundation
import Accelerate

/// Captures the microphone with AVCaptureSession, converts to 16 kHz mono, and
/// writes a 16-bit PCM WAV to disk **incrementally** — every buffer hits the
/// file as it arrives, so even a crash mid-recording keeps the audio.
///
/// AVAudioEngine was the original capture path but it cannot be pointed at a
/// chosen input device: setting the AUHAL's current device either makes the
/// engine refuse to start (-10868) or start against the previous device's
/// format and deliver no audio at all. AVCaptureSession selects a device by
/// unique ID as a first-class feature, which is why the capture lives here.
final class AudioRecorder: NSObject {
    let url: URL
    private let deviceUID: String?

    /// Called with the RMS level (0...1) of each buffer (feeds the waveform).
    var onLevel: ((Float) -> Void)?

    private let session = AVCaptureSession()
    private let output = AVCaptureAudioDataOutput()
    private let captureQueue = DispatchQueue(label: "com.jdrechsler.SpeechToText.capture")
    private var file: AVAudioFile?
    private var converter: AVAudioConverter?
    private var targetFormat: AVAudioFormat?
    private var startedAt: Date?
    /// Converted samples not yet consumed by a full level window. Only ever
    /// touched from the capture queue.
    private var levelTail: [Float] = []

    private static let targetSampleRate = 16_000.0

    /// One waveform level per 50 ms of audio, independent of how big the
    /// device's IO buffers are. A Bluetooth headset runs the input at 16 kHz
    /// and hands us ~4096-frame buffers, i.e. one callback every 256 ms — one
    /// level per callback made the wave crawl and stutter. Slicing in audio
    /// time keeps the resolution identical on every input device.
    private static let levelWindowFrames = 800

    init(url: URL, deviceUID: String? = nil) {
        self.url = url
        self.deviceUID = deviceUID
        super.init()
    }

    var duration: TimeInterval {
        startedAt.map { Date().timeIntervalSince($0) } ?? 0
    }

    func start() throws {
        guard let device = resolveDevice() else { throw RecorderError.noInputDevice }

        guard
            let targetFormat = AVAudioFormat(
                commonFormat: .pcmFormatFloat32,
                sampleRate: Self.targetSampleRate,
                channels: 1,
                interleaved: false)
        else {
            throw RecorderError.formatSetupFailed
        }
        self.targetFormat = targetFormat
        converter = nil

        let input: AVCaptureDeviceInput
        do {
            input = try AVCaptureDeviceInput(device: device)
        } catch {
            throw RecorderError.deviceSelectionFailed
        }

        session.beginConfiguration()
        guard session.canAddInput(input), session.canAddOutput(output) else {
            session.commitConfiguration()
            throw RecorderError.deviceSelectionFailed
        }
        session.addInput(input)
        output.setSampleBufferDelegate(self, queue: captureQueue)
        session.addOutput(output)
        session.commitConfiguration()

        // File encodes 16-bit PCM on disk; we hand it float buffers at 16 kHz.
        let settings: [String: Any] = [
            AVFormatIDKey: kAudioFormatLinearPCM,
            AVSampleRateKey: Self.targetSampleRate,
            AVNumberOfChannelsKey: 1,
            AVLinearPCMBitDepthKey: 16,
            AVLinearPCMIsFloatKey: false,
            AVLinearPCMIsBigEndianKey: false,
            AVLinearPCMIsNonInterleaved: false,
        ]
        file = try AVAudioFile(
            forWriting: url, settings: settings,
            commonFormat: .pcmFormatFloat32, interleaved: false)

        levelTail.removeAll(keepingCapacity: true)

        session.startRunning()
        startedAt = Date()
    }

    func stop() {
        // Returns only once the session has stopped, so no further sample
        // buffers can be in flight when the file is released below.
        session.stopRunning()
        output.setSampleBufferDelegate(nil, queue: nil)
        for input in session.inputs {
            session.removeInput(input)
        }
        file = nil  // releasing flushes and finalizes the WAV header
        converter = nil
        levelTail.removeAll(keepingCapacity: false)
    }

    private func resolveDevice() -> AVCaptureDevice? {
        if let deviceUID, let device = AVCaptureDevice(uniqueID: deviceUID) {
            return device
        }
        return AVCaptureDevice.default(for: .audio)
    }

    /// The device dictates the delivered format, so the converter is built from
    /// the first buffer and rebuilt if the device renegotiates mid-recording.
    private func converter(for inputFormat: AVAudioFormat) -> AVAudioConverter? {
        if let converter, converter.inputFormat == inputFormat { return converter }
        guard let targetFormat,
            let rebuilt = AVAudioConverter(from: inputFormat, to: targetFormat)
        else { return nil }
        converter = rebuilt
        return rebuilt
    }

    private func pcmBuffer(from sampleBuffer: CMSampleBuffer) -> AVAudioPCMBuffer? {
        guard
            let description = CMSampleBufferGetFormatDescription(sampleBuffer),
            let streamDescription = CMAudioFormatDescriptionGetStreamBasicDescription(description),
            let format = AVAudioFormat(streamDescription: streamDescription)
        else { return nil }

        let frames = AVAudioFrameCount(CMSampleBufferGetNumSamples(sampleBuffer))
        guard frames > 0,
            let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: frames)
        else { return nil }
        buffer.frameLength = frames

        guard
            CMSampleBufferCopyPCMDataIntoAudioBufferList(
                sampleBuffer, at: 0, frameCount: Int32(frames),
                into: buffer.mutableAudioBufferList) == noErr
        else { return nil }
        return buffer
    }

    private func process(_ buffer: AVAudioPCMBuffer) {
        guard let file, let converter = converter(for: buffer.format) else { return }

        let ratio = Self.targetSampleRate / buffer.format.sampleRate
        let capacity = AVAudioFrameCount(Double(buffer.frameLength) * ratio) + 64
        guard
            let converted = AVAudioPCMBuffer(
                pcmFormat: converter.outputFormat, frameCapacity: capacity)
        else { return }

        var supplied = false
        var conversionError: NSError?
        converter.convert(to: converted, error: &conversionError) { _, status in
            if supplied {
                status.pointee = .noDataNow
                return nil
            }
            supplied = true
            status.pointee = .haveData
            return buffer
        }
        guard conversionError == nil, converted.frameLength > 0 else { return }

        do {
            try file.write(from: converted)
        } catch {
            // Keep capturing; a single failed write shouldn't kill the session.
        }

        if let channel = converted.floatChannelData?[0] {
            emitLevels(channel, frameCount: Int(converted.frameLength))
        }
    }

    /// Emit one RMS level per `levelWindowFrames`, carrying the leftover
    /// samples into the next buffer so no audio is skipped or double-counted.
    private func emitLevels(_ samples: UnsafePointer<Float>, frameCount: Int) {
        levelTail.append(contentsOf: UnsafeBufferPointer(start: samples, count: frameCount))

        var consumed = 0
        while levelTail.count - consumed >= Self.levelWindowFrames {
            var rms: Float = 0
            levelTail.withUnsafeBufferPointer { buffer in
                guard let base = buffer.baseAddress else { return }
                vDSP_rmsqv(base + consumed, 1, &rms, vDSP_Length(Self.levelWindowFrames))
            }
            // Speech RMS is typically well below 1.0; scale up for a lively waveform.
            onLevel?(min(1, rms * 8))
            consumed += Self.levelWindowFrames
        }
        if consumed > 0 { levelTail.removeFirst(consumed) }
    }
}

extension AudioRecorder: AVCaptureAudioDataOutputSampleBufferDelegate {
    func captureOutput(
        _ output: AVCaptureOutput,
        didOutput sampleBuffer: CMSampleBuffer,
        from connection: AVCaptureConnection
    ) {
        guard let buffer = pcmBuffer(from: sampleBuffer) else { return }
        process(buffer)
    }
}

enum RecorderError: LocalizedError {
    case noInputDevice
    case formatSetupFailed
    case deviceSelectionFailed

    var errorDescription: String? {
        switch self {
        case .noInputDevice: return "No microphone input device available"
        case .formatSetupFailed: return "Could not set up audio conversion"
        case .deviceSelectionFailed: return "Could not switch to the selected microphone"
        }
    }
}
