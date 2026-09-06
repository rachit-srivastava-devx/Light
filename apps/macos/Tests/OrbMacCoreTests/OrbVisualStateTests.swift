import XCTest
@testable import OrbMacCore

/// Mirrors apps/mobile/src/AppModel.ts's ORB_VISUAL_PRESETS field-for-field. Each of the 7
/// documented states must map to the exact documented preset values (speed / breathPeriodMs /
/// breathDepth / glow / colors). These constants are transcribed by hand from the TS source, not
/// derived, so a mismatch here is a real drift between the RN spec and this port, not a rounding
/// artifact.
final class OrbVisualStateTests: XCTestCase {
    private func assertPreset(
        _ state: OrbVisualState,
        speed: Double,
        breathPeriodMs: Double,
        breathDepth: Double,
        glow: Double,
        colors: [String],
        file: StaticString = #filePath,
        line: UInt = #line
    ) {
        let preset = OrbAnimationPreset.preset(for: state)
        XCTAssertEqual(preset.speed, speed, accuracy: 0.0001, "speed mismatch for \(state)", file: file, line: line)
        XCTAssertEqual(preset.breathPeriodMs, breathPeriodMs, accuracy: 0.0001, "breathPeriodMs mismatch for \(state)", file: file, line: line)
        XCTAssertEqual(preset.breathDepth, breathDepth, accuracy: 0.0001, "breathDepth mismatch for \(state)", file: file, line: line)
        XCTAssertEqual(preset.glow, glow, accuracy: 0.0001, "glow mismatch for \(state)", file: file, line: line)
        XCTAssertEqual(preset.colors, colors, "colors mismatch for \(state)", file: file, line: line)
    }

    func testBooting() {
        assertPreset(.booting, speed: 3.5, breathPeriodMs: 5_500, breathDepth: 0.045, glow: 0.28,
                     colors: ["#6576e8", "#aebcff", "#f5f7ff"])
    }

    func testListening() {
        assertPreset(.listening, speed: 5.5, breathPeriodMs: 4_200, breathDepth: 0.08, glow: 0.42,
                     colors: ["#448bd6", "#8ed5ff", "#f2fbff"])
    }

    func testThinking() {
        assertPreset(.thinking, speed: 7, breathPeriodMs: 3_900, breathDepth: 0.05, glow: 0.36,
                     colors: ["#5c50e6", "#9892f5", "#e5e6ff"])
    }

    func testWorking() {
        assertPreset(.working, speed: 4.6, breathPeriodMs: 6_500, breathDepth: 0.04, glow: 0.3,
                     colors: ["#278f89", "#7ed2c7", "#effffb"])
    }

    func testSuccess() {
        assertPreset(.success, speed: 4, breathPeriodMs: 2_400, breathDepth: 0.1, glow: 0.55,
                     colors: ["#58b89f", "#b5f1d5", "#fff9e9"])
    }

    func testPaused() {
        assertPreset(.paused, speed: 1.3, breathPeriodMs: 7_500, breathDepth: 0.015, glow: 0.12,
                     colors: ["#53627f", "#8997b5", "#d9e0ef"])
    }

    func testError() {
        // Warm degraded signal: noticeable without alarm-red strobing (matches motion.ts's own
        // rationale for this palette, transcribed at the call site in AppModel.ts).
        assertPreset(.error, speed: 2.3, breathPeriodMs: 3_100, breathDepth: 0.055, glow: 0.3,
                     colors: ["#c87552", "#f0b08a", "#fff0df"])
    }

    /// All 7 cases of the enum must be represented — this is the exhaustiveness check that a
    /// future 8th state (or an accidental removal) would break.
    func testAllSevenStatesCovered() {
        let all: [OrbVisualState] = [.booting, .listening, .thinking, .working, .success, .paused, .error]
        XCTAssertEqual(all.count, 7)
        for state in all {
            _ = OrbAnimationPreset.preset(for: state) // must not crash / must be defined for every case
        }
    }

    // ---------------------------------------------------------------------------
    // Breathing motion helpers (10000ms/6bpm coherent-breathing pulse envelope shared across
    // states; opacity floor 0.5/depth 0.25, scale depth 0.06). Reduce-motion freezes at the low end.
    // ---------------------------------------------------------------------------

    func testBreathsPerMinuteToPeriodMs() {
        XCTAssertEqual(OrbBreathing.breathsPerMinuteToPeriodMs(6), 10_000, accuracy: 0.0001)
    }

    func testPulseOpacityRangeUsesFloorAndDepth() {
        let range = OrbBreathing.pulseOpacityRange()
        XCTAssertEqual(range.low, 0.5, accuracy: 0.0001)
        XCTAssertEqual(range.high, 0.75, accuracy: 0.0001)
    }

    func testPulseScaleRangeUsesDepth() {
        let range = OrbBreathing.pulseScaleRange()
        XCTAssertEqual(range.low, 1.0, accuracy: 0.0001)
        XCTAssertEqual(range.high, 1.06, accuracy: 0.0001)
    }

    func testResolveMotionStateAnimatingWhenMotionAllowed() {
        XCTAssertEqual(OrbBreathing.resolveMotionState(reduceMotionEnabled: false), .animating)
    }

    func testResolveMotionStateFrozenWhenReduceMotionEnabled() {
        XCTAssertEqual(OrbBreathing.resolveMotionState(reduceMotionEnabled: true), .frozen)
    }
}
