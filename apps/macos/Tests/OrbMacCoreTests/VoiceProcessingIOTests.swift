import XCTest
@testable import OrbMacCore

/// Tests for the VoiceProcessingIO manager and AEC wiring.
///
/// **What IS testable without hardware:**
/// - VPIO AudioUnit creation (component found + instance created)
/// - MicCapture accepts the `voiceProcessingEnabled` flag
/// - VoiceProcessingIOManager.isAvailable reflects platform truth
///
/// **What is NOT testable in CI (honest gap):**
/// - Actual AEC effectiveness (requires a human speaking while TTS plays
///   simultaneously on real hardware with real mic permission)
/// - True echo cancellation quality (requires a live mic + speaker loop)
/// - The VPIO render callback delivering real audio frames (requires the
///   audio unit to actually start, which needs a real audio device)
///
/// See `docs/lane-contracts/F36-aec-vpio-wiring.md` for the full acceptance
/// criteria and what requires human-in-the-loop verification.
final class VoiceProcessingIOTests: XCTestCase {

    // MARK: - VoiceProcessingIOManager

    func testVPIOIsAvailableOnMacOS() {
        // On macOS 13+, kAudioUnitSubType_VoiceProcessingIO should be available.
        // If this test fails, the platform does not support VPIO and all
        // voice-processing tests should skip.
        XCTAssertTrue(VoiceProcessingIOManager.isAvailable,
                      "VPIO AudioUnit not available — voice processing tests will skip")
    }

    func testVPIOAudioUnitCanBeCreated() throws {
        guard VoiceProcessingIOManager.isAvailable else {
            throw XCTSkip("VPIO not available on this platform")
        }
        let manager = try VoiceProcessingIOManager()
        // Manager created successfully — proves the AudioUnit instantiated.
        // Stop immediately to release the hardware resource.
        manager.stop()
    }

    func testVPIOAudioUnitStartAndStopWithoutCrash() throws {
        guard VoiceProcessingIOManager.isAvailable else {
            throw XCTSkip("VPIO not available on this platform")
        }
        let manager = try VoiceProcessingIOManager()
        do {
            try manager.start()
            // If start succeeds, stop cleanly. If start fails (no audio
            // device in CI), that's expected — verify no crash.
        } catch {
            // Start may fail in headless CI — this is expected and honest.
            // The test proves the code path runs without crashing.
        }
        manager.stop()
    }

    func testVPIOStopWithoutStartIsSafeNoOp() throws {
        guard VoiceProcessingIOManager.isAvailable else {
            throw XCTSkip("VPIO not available on this platform")
        }
        let manager = try VoiceProcessingIOManager()
        manager.stop() // must not crash
    }

    func testVPIOOnProcessedFrameIsSettable() throws {
        guard VoiceProcessingIOManager.isAvailable else {
            throw XCTSkip("VPIO not available on this platform")
        }
        let manager = try VoiceProcessingIOManager()
        var received: Data?
        manager.onProcessedFrame = { data in received = data }
        XCTAssertNil(received)
        XCTAssertNotNil(manager.onProcessedFrame)
        manager.stop()
    }

    // MARK: - MicCapture voice processing flag

    func testMicCaptureDefaultsToNoVoiceProcessing() {
        let capture = MicCapture()
        XCTAssertFalse(capture.isVoiceProcessingEnabled)
    }

    func testMicCaptureAcceptsVoiceProcessingEnabled() {
        let capture = MicCapture(voiceProcessingEnabled: true)
        if VoiceProcessingIOManager.isAvailable {
            XCTAssertTrue(capture.isVoiceProcessingEnabled)
        } else {
            // Platform doesn't support VPIO — flag should gracefully degrade
            XCTAssertFalse(capture.isVoiceProcessingEnabled)
        }
    }

    func testMicCaptureVoiceProcessingDisabled() {
        let capture = MicCapture(voiceProcessingEnabled: false)
        XCTAssertFalse(capture.isVoiceProcessingEnabled)
    }

    func testMicCaptureVoiceProcessingFallbackWhenUnavailable() {
        // Even if we request voice processing, if the platform doesn't
        // support it, the flag should be false.
        let capture = MicCapture(voiceProcessingEnabled: true)
        if !VoiceProcessingIOManager.isAvailable {
            XCTAssertFalse(capture.isVoiceProcessingEnabled,
                           "Should gracefully degrade when VPIO is unavailable")
        }
    }

    func testMicCaptureWithVoiceProcessingStopWithoutStartIsSafeNoOp() {
        let capture = MicCapture(voiceProcessingEnabled: true)
        capture.stop() // must not crash regardless of voice processing state
    }

    func testMicCaptureWithVoiceProcessingOnFrameIsSettable() {
        let capture = MicCapture(voiceProcessingEnabled: true)
        var received: Data?
        capture.onFrame = { data in received = data }
        XCTAssertNil(received)
        XCTAssertNotNil(capture.onFrame)
    }

    // MARK: - MicCaptureError Equatable

    func testMicCaptureErrorEquality() {
        XCTAssertEqual(MicCaptureError.formatConstructionFailed,
                       MicCaptureError.formatConstructionFailed)
        XCTAssertNotEqual(MicCaptureError.formatConstructionFailed,
                          MicCaptureError.noInputRoute(channels: 0, sampleRate: 0))
        XCTAssertEqual(MicCaptureError.noInputRoute(channels: 2, sampleRate: 44100),
                       MicCaptureError.noInputRoute(channels: 2, sampleRate: 44100))
    }

    // MARK: - VoiceProcessingError Equatable

    func testVoiceProcessingErrorEquality() throws {
        guard VoiceProcessingIOManager.isAvailable else {
            throw XCTSkip("VPIO not available on this platform")
        }
        XCTAssertEqual(VoiceProcessingError.vpioNotAvailable,
                       VoiceProcessingError.vpioNotAvailable)
        XCTAssertNotEqual(VoiceProcessingError.vpioNotAvailable,
                          VoiceProcessingError.componentNotFound)
        XCTAssertEqual(VoiceProcessingError.unitCreationFailed(noErr),
                       VoiceProcessingError.unitCreationFailed(noErr))
        XCTAssertNotEqual(VoiceProcessingError.unitCreationFailed(noErr),
                          VoiceProcessingError.unitCreationFailed(-1))
    }
}
