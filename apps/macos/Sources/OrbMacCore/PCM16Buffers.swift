import AVFoundation
import Foundation

/// Pure helpers for converting between raw PCM16 `Data` / `[Int16]` and
/// `AVAudioPCMBuffer`, and for running one `AVAudioConverter` pass. These are
/// the building blocks `MicCapture` and `TTSPlayback` wire into a real
/// `AVAudioEngine` tap/render graph — kept separate so the conversion math
/// and byte layout are unit-testable with synthetic buffers, no hardware
/// input/output device required.
public enum PCM16Buffers {
    /// Builds a mono PCM16 `AVAudioFormat` at the given sample rate
    /// (interleaved — irrelevant for mono but required by the initializer).
    public static func monoInt16Format(sampleRate: Double) -> AVAudioFormat? {
        AVAudioFormat(commonFormat: .pcmFormatInt16, sampleRate: sampleRate, channels: 1, interleaved: true)
    }

    /// Packs raw little-endian Int16 mono samples into an `AVAudioPCMBuffer`
    /// of the given format. `data.count` must be a whole number of Int16
    /// samples (even byte count); a trailing odd byte, if any, is dropped
    /// rather than corrupting the last sample.
    public static func makeBuffer(from data: Data, format: AVAudioFormat) -> AVAudioPCMBuffer? {
        precondition(format.commonFormat == .pcmFormatInt16, "expects an Int16 PCM format")
        let frameCount = AVAudioFrameCount(data.count / 2)
        guard frameCount > 0, let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: frameCount) else {
            return nil
        }
        buffer.frameLength = frameCount
        guard let channelData = buffer.int16ChannelData else { return nil }
        data.withUnsafeBytes { (raw: UnsafeRawBufferPointer) in
            let samples = raw.bindMemory(to: Int16.self)
            for i in 0..<Int(frameCount) {
                channelData[0][i] = samples[i]
            }
        }
        return buffer
    }

    /// Reads the mono Int16 samples out of a PCM16 `AVAudioPCMBuffer` as a
    /// plain array (order preserved, one entry per frame).
    public static func int16Samples(from buffer: AVAudioPCMBuffer) -> [Int16] {
        guard buffer.format.commonFormat == .pcmFormatInt16,
              let channelData = buffer.int16ChannelData else { return [] }
        return Array(UnsafeBufferPointer(start: channelData[0], count: Int(buffer.frameLength)))
    }

    /// Runs one `AVAudioConverter` pass over a single input buffer (no
    /// internal buffering of its own — each call supplies the converter
    /// exactly one buffer's worth of input, matching how a single
    /// AVAudioEngine tap callback or one arrived TTS chunk is converted).
    /// Returns the converted mono Int16 samples, or `nil` if the conversion
    /// failed or produced no output.
    public static func convert(
        _ inputBuffer: AVAudioPCMBuffer,
        using converter: AVAudioConverter
    ) -> [Int16]? {
        let targetFormat = converter.outputFormat
        let targetCapacity = AVAudioFrameCount(
            ceil(Double(inputBuffer.frameLength) * targetFormat.sampleRate / inputBuffer.format.sampleRate)
        ) + 2
        guard let outBuffer = AVAudioPCMBuffer(pcmFormat: targetFormat, frameCapacity: targetCapacity) else {
            return nil
        }
        var error: NSError?
        var suppliedInput = false
        let inputBlock: AVAudioConverterInputBlock = { _, outStatus in
            guard !suppliedInput else {
                outStatus.pointee = .noDataNow
                return nil
            }
            suppliedInput = true
            outStatus.pointee = .haveData
            return inputBuffer
        }
        converter.convert(to: outBuffer, error: &error, withInputFrom: inputBlock)
        if error != nil || outBuffer.frameLength == 0 { return nil }
        return int16Samples(from: outBuffer)
    }
}
