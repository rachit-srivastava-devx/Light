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

    private var engine: AVAudioEngine?
    private let accumulator = PCM16FrameAccumulator(frameSampleCount: AudioIOFormat.micFrameSampleCount)

    public init() {}

    /// Starts capture. Builds a fresh `AVAudioEngine` each call (mirroring
    /// OrbMic.swift: `inputNode` snapshots the hardware input format on
    /// first access, so a stored/reused engine can pin a stale format).
    /// Throws if the hardware input format is unusable (no input route) or
    /// if `engine.start()` fails — including the case where the process has
    /// not yet been granted microphone permission by the user (macOS surfaces
    /// this as a start failure / silent zero-format route rather than a
    /// distinct error type).
    public func start() throws {
        accumulator.reset()

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
