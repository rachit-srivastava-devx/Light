import AVFoundation
import Foundation

/// Native TTS audio playback for macOS: takes a stream of 24kHz mono PCM16
/// binary chunks (no WAV header, no fixed chunk size guarantee) and schedules
/// each one for playback as it arrives, so audio starts on the first chunk
/// rather than waiting for all chunks / a "speech_complete" signal. This is
/// what keeps the low-latency streaming behavior the relay/sidecar side
/// (defect A7) produces intact all the way to the speaker — buffering chunks
/// before playing would undo that fix on the client. Pure audio I/O: zero
/// WebSocket/protocol knowledge.
public final class TTSPlayback {
    public private(set) var isRunning = false

    private let engine = AVAudioEngine()
    private let player = AVAudioPlayerNode()
    private let sourceFormat: AVAudioFormat
    private var converter: AVAudioConverter?

    public init() {
        guard let sourceFormat = PCM16Buffers.monoInt16Format(sampleRate: AudioIOFormat.ttsSampleRate) else {
            fatalError("could not construct the 24kHz mono PCM16 TTS source format")
        }
        self.sourceFormat = sourceFormat
        engine.attach(player)
    }

    /// Starts the engine and connects the player node to the engine's own
    /// output format (whatever the default output device natively runs at),
    /// building the one converter used for every scheduled chunk.
    public func start() throws {
        let outputFormat = engine.outputNode.outputFormat(forBus: 0)
        engine.connect(player, to: engine.mainMixerNode, format: outputFormat)
        guard let converter = AVAudioConverter(from: sourceFormat, to: outputFormat) else {
            throw TTSPlaybackError.converterConstructionFailed
        }
        self.converter = converter
        engine.prepare()
        try engine.start()
        player.play()
        isRunning = true
    }

    /// Converts one raw PCM16 chunk to the engine's output format and
    /// schedules it immediately on the player node. No buffering across
    /// calls — each chunk is independent, which is what lets the first chunk
    /// start playing without waiting for the rest of the stream.
    public func scheduleChunk(_ pcm16: Data) {
        guard isRunning, let converter else { return }
        guard let sourceBuffer = PCM16Buffers.makeBuffer(from: pcm16, format: sourceFormat) else { return }
        guard let outputBuffer = Self.convertToOutputFormat(sourceBuffer, using: converter) else { return }
        player.scheduleBuffer(outputBuffer, completionHandler: nil)
    }

    public func stop() {
        player.stop()
        engine.stop()
        converter = nil
        isRunning = false
    }

    /// Like `PCM16Buffers.convert`, but returns the engine-format buffer
    /// itself (not just the samples) since `scheduleBuffer` needs the buffer.
    private static func convertToOutputFormat(
        _ inputBuffer: AVAudioPCMBuffer,
        using converter: AVAudioConverter
    ) -> AVAudioPCMBuffer? {
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
        return outBuffer
    }
}

public enum TTSPlaybackError: Error, Equatable {
    case converterConstructionFailed
}
