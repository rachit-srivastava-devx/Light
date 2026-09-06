import XCTest
@testable import OrbMacCore

/// Pure-logic tests for the accumulate-into-exact-320-sample-chunks
/// behavior: feeds irregular-sized input chunks (as real AVAudioEngine tap
/// buffers would after conversion) and asserts the output is always exactly
/// `frameSampleCount`-sample (640-byte at the default 320) frames, in order,
/// with no samples dropped or duplicated — the core invariant the brief
/// calls out as unit-testable without real hardware.
final class PCM16FrameAccumulatorTests: XCTestCase {
    func testExactMultipleProducesNoLeftover() {
        let acc = PCM16FrameAccumulator(frameSampleCount: 320)
        let samples = (0..<640).map { Int16($0 % 100) }
        let frames = acc.append(samples)
        XCTAssertEqual(frames.count, 2)
        XCTAssertEqual(frames[0].count, 640) // 320 samples * 2 bytes
        XCTAssertEqual(frames[1].count, 640)
        XCTAssertEqual(acc.pendingSampleCount, 0)
    }

    func testIrregularChunkSizesStillProduceExactFrames() {
        // Mirrors the real-world case the brief describes: tap buffer sizes
        // never align to exactly 320 samples.
        let acc = PCM16FrameAccumulator(frameSampleCount: 320)
        var allSamples: [Int16] = []
        var emittedFrames: [Data] = []

        let chunkSizes = [100, 300, 250, 500, 90, 1024, 1]
        for size in chunkSizes {
            let chunk = (0..<size).map { _ in Int16.random(in: Int16.min...Int16.max) }
            allSamples.append(contentsOf: chunk)
            emittedFrames.append(contentsOf: acc.append(chunk))
        }

        // Every emitted frame must be exactly 640 bytes.
        for frame in emittedFrames {
            XCTAssertEqual(frame.count, 640)
        }

        // No drops/duplicates: reconstruct the emitted samples + whatever is
        // still pending and confirm it equals the total input, in order.
        var reconstructed: [Int16] = []
        for frame in emittedFrames {
            frame.withUnsafeBytes { raw in
                let samples = raw.bindMemory(to: Int16.self)
                reconstructed.append(contentsOf: samples)
            }
        }
        let expectedFullFrames = allSamples.count / 320
        XCTAssertEqual(emittedFrames.count, expectedFullFrames)
        XCTAssertEqual(acc.pendingSampleCount, allSamples.count % 320)
        XCTAssertEqual(reconstructed, Array(allSamples.prefix(expectedFullFrames * 320)))
    }

    func testByteLayoutIsLittleEndianInt16() {
        let acc = PCM16FrameAccumulator(frameSampleCount: 2)
        let frames = acc.append([Int16(1), Int16(-1)])
        XCTAssertEqual(frames.count, 1)
        let bytes = [UInt8](frames[0])
        // Int16(1) little-endian -> [0x01, 0x00]
        XCTAssertEqual(bytes[0], 0x01)
        XCTAssertEqual(bytes[1], 0x00)
        // Int16(-1) little-endian -> [0xFF, 0xFF]
        XCTAssertEqual(bytes[2], 0xFF)
        XCTAssertEqual(bytes[3], 0xFF)
    }

    func testAppendAcrossCallsPreservesOrderAtBoundary() {
        let acc = PCM16FrameAccumulator(frameSampleCount: 4)
        var frames: [Data] = []
        frames.append(contentsOf: acc.append([1, 2, 3])) // 3 pending, no frame yet
        XCTAssertEqual(frames.count, 0)
        XCTAssertEqual(acc.pendingSampleCount, 3)
        frames.append(contentsOf: acc.append([4, 5, 6]))
        // First 4 (1,2,3,4) should form the first frame; 5,6 remain pending.
        XCTAssertEqual(frames.count, 1)
        XCTAssertEqual(acc.pendingSampleCount, 2)
        let samples = frames[0].withUnsafeBytes { raw -> [Int16] in
            Array(raw.bindMemory(to: Int16.self))
        }
        XCTAssertEqual(samples, [1, 2, 3, 4])
    }

    func testResetDiscardsPendingSamples() {
        let acc = PCM16FrameAccumulator(frameSampleCount: 10)
        _ = acc.append([1, 2, 3])
        XCTAssertEqual(acc.pendingSampleCount, 3)
        acc.reset()
        XCTAssertEqual(acc.pendingSampleCount, 0)
    }
}
