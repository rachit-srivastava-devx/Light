import Foundation

/// Accumulates mono Int16 samples arriving in irregularly-sized chunks (as
/// AVAudioEngine tap buffers do — a callback is never guaranteed to hand over
/// exactly one 20ms frame's worth of samples) and emits complete
/// `frameSampleCount`-sample frames as raw little-endian `Data`, in order,
/// with no samples dropped or duplicated across calls.
///
/// Pure logic, no AVFoundation dependency — this is what makes the
/// accumulate-into-exact-frames behavior unit-testable without real
/// microphone hardware. `MicCapture` (AudioCapture.swift) feeds converted
/// samples through this on every tap callback.
public final class PCM16FrameAccumulator {
    public let frameSampleCount: Int
    private var pending: [Int16] = []

    public init(frameSampleCount: Int = AudioIOFormat.micFrameSampleCount) {
        precondition(frameSampleCount > 0, "frameSampleCount must be positive")
        self.frameSampleCount = frameSampleCount
    }

    /// Number of samples buffered but not yet emitted as a complete frame.
    public var pendingSampleCount: Int { pending.count }

    /// Appends new samples and returns zero or more complete frames, each
    /// exactly `frameSampleCount * 2` bytes (Int16 little-endian, mono).
    /// Order is preserved across calls; leftover samples remain buffered for
    /// the next call.
    @discardableResult
    public func append(_ samples: [Int16]) -> [Data] {
        pending.append(contentsOf: samples)
        var frames: [Data] = []
        while pending.count >= frameSampleCount {
            let frameSamples = Array(pending.prefix(frameSampleCount))
            pending.removeFirst(frameSampleCount)
            frames.append(frameSamples.withUnsafeBytes { Data($0) })
        }
        return frames
    }

    /// Discards any buffered-but-incomplete samples (e.g. on stop()).
    public func reset() {
        pending.removeAll()
    }
}
