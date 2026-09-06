import AVFoundation
import XCTest
@testable import OrbMacCore

/// `TTSPlayback.start()` needs a real default output device (AVAudioEngine
/// output rendering); unlike microphone capture it does not need an
/// interactive permission grant, so it is attempted for real here rather
/// than skipped outright. If this environment genuinely has no usable
/// output device, the test skips itself with an honest message instead of
/// silently passing or faking success — see the evidence report.
final class TTSPlaybackTests: XCTestCase {
    func testStartScheduleChunkStopDoesNotThrowOnRealOutputDevice() throws {
        let playback = TTSPlayback()
        do {
            try playback.start()
        } catch {
            throw XCTSkip("no usable audio output device in this environment: \(error)")
        }
        XCTAssertTrue(playback.isRunning)

        // 100ms of 24kHz mono PCM16 silence == 2400 samples == 4800 bytes,
        // an arbitrary chunk size (no fixed-chunk-size guarantee per spec).
        let sampleCount = 2_400
        let silence = [Int16](repeating: 0, count: sampleCount)
        let chunk = silence.withUnsafeBytes { Data($0) }

        playback.scheduleChunk(chunk) // must not throw / crash
        playback.stop()
        XCTAssertFalse(playback.isRunning)
    }

    func testScheduleChunkBeforeStartIsSafeNoOp() {
        let playback = TTSPlayback()
        let chunk = [Int16](repeating: 0, count: 10).withUnsafeBytes { Data($0) }
        playback.scheduleChunk(chunk) // not running yet — must not crash
        XCTAssertFalse(playback.isRunning)
    }

    func testMakeBufferFromArbitraryChunkSizeRoundTrips() throws {
        // Exercises the "no fixed chunk size guarantee" requirement directly
        // against the pure buffer-construction helper, independent of
        // whether a real output device is available.
        let format = try XCTUnwrap(PCM16Buffers.monoInt16Format(sampleRate: AudioIOFormat.ttsSampleRate))
        for sampleCount in [1, 7, 2_400, 4_096] {
            let samples = (0..<sampleCount).map { Int16($0 % 30_000) }
            let data = samples.withUnsafeBytes { Data($0) }
            let buffer = try XCTUnwrap(PCM16Buffers.makeBuffer(from: data, format: format))
            XCTAssertEqual(Int(buffer.frameLength), sampleCount)
            XCTAssertEqual(PCM16Buffers.int16Samples(from: buffer), samples)
        }
    }
}
