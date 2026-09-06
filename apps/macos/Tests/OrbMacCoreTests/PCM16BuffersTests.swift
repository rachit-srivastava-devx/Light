import AVFoundation
import XCTest
@testable import OrbMacCore

/// Tests the PCM format conversion math and byte layout against known,
/// synthetic `AVAudioPCMBuffer`s — no real microphone/speaker hardware, and
/// no interactive permission prompt, since these buffers are constructed
/// in-process from plain sample arrays.
final class PCM16BuffersTests: XCTestCase {
    func testMakeBufferPacksInt16SamplesInOrder() throws {
        let format = try XCTUnwrap(PCM16Buffers.monoInt16Format(sampleRate: 16_000))
        let samples: [Int16] = [1, -1, 32767, -32768, 0]
        let data = samples.withUnsafeBytes { Data($0) }
        let buffer = try XCTUnwrap(PCM16Buffers.makeBuffer(from: data, format: format))
        XCTAssertEqual(buffer.frameLength, AVAudioFrameCount(samples.count))
        XCTAssertEqual(PCM16Buffers.int16Samples(from: buffer), samples)
    }

    func testMakeBufferDropsTrailingOddByteRatherThanCorrupting() throws {
        let format = try XCTUnwrap(PCM16Buffers.monoInt16Format(sampleRate: 16_000))
        let samples: [Int16] = [10, 20, 30]
        var data = samples.withUnsafeBytes { Data($0) }
        data.append(0xAB) // one trailing odd byte
        let buffer = try XCTUnwrap(PCM16Buffers.makeBuffer(from: data, format: format))
        XCTAssertEqual(Int(buffer.frameLength), 3)
        XCTAssertEqual(PCM16Buffers.int16Samples(from: buffer), samples)
    }

    func testMakeBufferReturnsNilForEmptyData() {
        let format = PCM16Buffers.monoInt16Format(sampleRate: 16_000)!
        XCTAssertNil(PCM16Buffers.makeBuffer(from: Data(), format: format))
    }

    func testConvertSameSampleRateIsIdentityForFullScaleValue() throws {
        // Same-rate conversion (no resampling filter) makes the output value
        // deterministic: converting Float32 1.0 (full scale) to Int16 must
        // produce Int16.max (32767), and -1.0 must produce Int16.min.
        let floatFormat = try XCTUnwrap(
            AVAudioFormat(commonFormat: .pcmFormatFloat32, sampleRate: 16_000, channels: 1, interleaved: false)
        )
        let int16Format = try XCTUnwrap(PCM16Buffers.monoInt16Format(sampleRate: 16_000))
        let frameCount: AVAudioFrameCount = 4
        let inputBuffer = try XCTUnwrap(AVAudioPCMBuffer(pcmFormat: floatFormat, frameCapacity: frameCount))
        inputBuffer.frameLength = frameCount
        let channel = try XCTUnwrap(inputBuffer.floatChannelData)
        channel[0][0] = 1.0
        channel[0][1] = -1.0
        channel[0][2] = 0.0
        channel[0][3] = 0.5

        let converter = try XCTUnwrap(AVAudioConverter(from: floatFormat, to: int16Format))
        let output = try XCTUnwrap(PCM16Buffers.convert(inputBuffer, using: converter))

        XCTAssertEqual(output.count, 4)
        XCTAssertEqual(output[0], Int16.max)
        XCTAssertEqual(output[1], Int16.min)
        XCTAssertEqual(output[2], 0)
        // 0.5 full-scale -> ~16383/16384, allow a 1-unit rounding tolerance.
        XCTAssertTrue(abs(Int(output[3]) - 16384) <= 1, "expected ~16384, got \(output[3])")
    }

    func testConvertDownsamplingProducesRoughlyExpectedSampleCount() throws {
        // 48kHz -> 16kHz is an exact 3:1 ratio; assert the converted sample
        // count is close to input/3 (resampling filters can shift the exact
        // count by a few samples at buffer edges, so allow tolerance).
        let sourceFormat = try XCTUnwrap(
            AVAudioFormat(commonFormat: .pcmFormatFloat32, sampleRate: 48_000, channels: 1, interleaved: false)
        )
        let targetFormat = try XCTUnwrap(PCM16Buffers.monoInt16Format(sampleRate: 16_000))
        let frameCount: AVAudioFrameCount = 1024
        let inputBuffer = try XCTUnwrap(AVAudioPCMBuffer(pcmFormat: sourceFormat, frameCapacity: frameCount))
        inputBuffer.frameLength = frameCount
        let channel = try XCTUnwrap(inputBuffer.floatChannelData)
        for i in 0..<Int(frameCount) { channel[0][i] = 0 } // silence

        let converter = try XCTUnwrap(AVAudioConverter(from: sourceFormat, to: targetFormat))
        let output = try XCTUnwrap(PCM16Buffers.convert(inputBuffer, using: converter))

        let expected = Double(frameCount) * 16_000.0 / 48_000.0
        XCTAssertTrue(abs(Double(output.count) - expected) <= 8, "expected ~\(expected), got \(output.count)")
        // Silence in, silence out.
        XCTAssertTrue(output.allSatisfy { $0 == 0 })
    }
}
