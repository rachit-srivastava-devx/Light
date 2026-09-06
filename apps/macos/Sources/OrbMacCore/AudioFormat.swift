import Foundation

/// Shared audio-format constants for the mac client, matching the relay
/// protocol exactly (same numbers as ios/OrbMobile/OrbMic.swift and the
/// Android OrbMicModule.kt it mirrors):
///   - mic capture: 16kHz mono PCM16, 20ms/320-sample/640-byte frames.
///   - TTS playback: 24kHz mono PCM16 streamed as arbitrary-sized chunks.
public enum AudioIOFormat {
    public static let micSampleRate: Double = 16_000
    /// 20ms at 16kHz = 320 samples = 640 bytes (Int16 mono), matching the
    /// wire frame size the relay expects for raw binary WebSocket frames.
    public static let micFrameSampleCount = 320
    public static let micFrameByteCount = micFrameSampleCount * 2

    public static let ttsSampleRate: Double = 24_000
}
