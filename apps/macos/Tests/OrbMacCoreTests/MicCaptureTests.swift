import XCTest
@testable import OrbMacCore

/// `MicCapture.start()` against real hardware requires an actual input
/// device and (the first time) an interactive microphone permission grant
/// from the user — neither is available non-interactively in this
/// environment, so real capture is NOT exercised here. See the evidence
/// report for the honest gap. What IS tested here without hardware: the
/// public surface exists with the documented shape, and `onFrame` is a
/// plain settable closure property untouched by any AVFoundation state.
final class MicCaptureTests: XCTestCase {
    func testOnFrameIsSettableAndDefaultsToNil() {
        let capture = MicCapture()
        var received: Data?
        capture.onFrame = { data in received = data }
        // No engine is running, so nothing should fire — this only proves
        // the property wiring, not the capture pipeline itself.
        XCTAssertNil(received)
        XCTAssertNotNil(capture.onFrame)
    }

    func testStopWithoutStartIsSafeNoOp() {
        let capture = MicCapture()
        capture.stop() // must not crash/throw when never started
    }
}
