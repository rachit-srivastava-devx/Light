import AVFoundation
import Foundation

/// Native microphone capture for macOS: delivers 16kHz mono PCM16, exactly
/// 640-byte (320-sample, ~20ms) frames via `onFrame`, ready to send as raw
/// binary WebSocket frames by whichever unit owns the protocol client (this
/// type has zero WebSocket/protocol knowledge — pure audio I/O).
///
/// Algorithmically mirrors ios/OrbMobile/OrbMic.swift's accumulate-then-slice
/// pattern (same target format, same 320-sample frame size, same
/// build-the-converter-from-the-first-delivered-buffer's-own-format
/// approach), but macOS has no `AVAudioSession` category concept — there is
/// no session to configure, just `AVAudioEngine.inputNode` against whatever
/// the system's default input device is.
public final class MicCapture {
    /// Called on an internal audio thread (not the main queue) once per
    /// complete 640-byte frame. Callers that touch UI/main-actor state must
    /// hop back to the main queue themselves.
    public var onFrame: ((Data) -> Void)?

    /// Whether voice processing (hardware AEC via CoreAudio VPIO) is enabled.
    /// When true and `VoiceProcessingIOManager.isAvailable`, the capture path
    /// uses the VPIO AudioUnit instead of a plain `AVAudioEngine.inputNode`.
    public private(set) var isVoiceProcessingEnabled: Bool

    private var engine: AVAudioEngine?
    private var vpioManager: VoiceProcessingIOManager?
    private let accumulator = PCM16FrameAccumulator(frameSampleCount: AudioIOFormat.micFrameSampleCount)

    public init() {
        self.isVoiceProcessingEnabled = false
    }

    /// - Parameter voiceProcessingEnabled: When `true` and
    ///   `VoiceProcessingIOManager.isAvailable`, hardware AEC is activated
    ///   via the CoreAudio VoiceProcessingIO AudioUnit. On macOS,
    ///   `AVAudioEngine.inputNode`'s voice-processing properties are
    ///   read-only, so the raw VPIO unit is the only path to real AEC.
    public init(voiceProcessingEnabled: Bool) {
        if voiceProcessingEnabled && VoiceProcessingIOManager.isAvailable {
            self.isVoiceProcessingEnabled = true
        } else {
            self.isVoiceProcessingEnabled = false
        }
    }

    /// Starts capture. Builds a fresh `AVAudioEngine` each call (mirroring
    /// OrbMic.swift: `inputNode` snapshots the hardware input format on
    /// first access, so a stored/reused engine can pin a stale format).
    /// When `voiceProcessingEnabled` was set to `true` in the initializer
    /// and `VoiceProcessingIOManager.isAvailable`, the VPIO AudioUnit is
    /// used instead of the plain AVAudioEngine input tap.
    /// Throws if the hardware input format is unusable (no input route) or
    /// if the engine/VPIO fails to start.
    public func start() throws {
        accumulator.reset()

        if isVoiceProcessingEnabled {
            try startWithVoiceProcessing()
        } else {
            try startWithPlainEngine()
        }
    }

    /// Starts capture using the VPIO AudioUnit for hardware AEC.
    private func startWithVoiceProcessing() throws {
        let manager = try VoiceProcessingIOManager()
        self.vpioManager = manager

        // The VPIO manager handles AEC/AGC internally. We wire its
        // processed output to our accumulator → onFrame pipeline.
        manager.onProcessedFrame = { [weak self] data in
            guard let self else { return }
            let samples = data.withUnsafeBytes { raw -> [Int16] in
                let ptr = raw.bindMemory(to: Int16.self)
                return Array(ptr)
            }
            for frame in self.accumulator.append(samples) {
                self.onFrame?(frame)
            }
        }

        try manager.start()
    }

    /// Starts capture using a plain AVAudioEngine (no AEC).
    private func startWithPlainEngine() throws {
        guard let targetFormat = PCM16Buffers.monoInt16Format(sampleRate: AudioIOFormat.micSampleRate) else {
            throw MicCaptureError.formatConstructionFailed
        }

        let engine = AVAudioEngine()
        self.engine = engine
        let input = engine.inputNode
        // `inputFormat(forBus:)` is the hardware's actual input format;
        // `outputFormat(forBus:)` stays uninitialized (0Hz) until the engine
        // runs. A 0ch/0Hz input format means no usable input route exists
        // yet (no mic permission granted, or no input device) — fail
        // cleanly rather than handing an invalid format to `installTap`,
        // which throws an uncatchable ObjC NSException on a format mismatch.
        let inputFormat = input.inputFormat(forBus: 0)
        guard inputFormat.channelCount > 0, inputFormat.sampleRate > 0 else {
            self.engine = nil
            throw MicCaptureError.noInputRoute(channels: inputFormat.channelCount, sampleRate: inputFormat.sampleRate)
        }

        var converter: AVAudioConverter?
        var converterSourceFormat: AVAudioFormat?

        // `format: nil` makes the engine use the input node's own format, so
        // a format mismatch (the uncatchable-NSException hazard above) is
        // structurally impossible here.
        input.installTap(onBus: 0, bufferSize: 1024, format: nil) { [weak self] buffer, _ in
            guard let self else { return }
            if converterSourceFormat != buffer.format {
                converter = AVAudioConverter(from: buffer.format, to: targetFormat)
                converterSourceFormat = buffer.format
            }
            guard let converter, let samples = PCM16Buffers.convert(buffer, using: converter) else { return }
            for frame in self.accumulator.append(samples) {
                self.onFrame?(frame)
            }
        }

        engine.prepare()
        do {
            try engine.start()
        } catch {
            input.removeTap(onBus: 0)
            self.engine = nil
            throw error
        }
    }

    public func stop() {
        vpioManager?.stop()
        vpioManager = nil
        engine?.inputNode.removeTap(onBus: 0)
        engine?.stop()
        engine = nil
        accumulator.reset()
    }
}

public enum MicCaptureError: Error, Equatable {
    case formatConstructionFailed
    case noInputRoute(channels: AVAudioChannelCount, sampleRate: Double)
}
