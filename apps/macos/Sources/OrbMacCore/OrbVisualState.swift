/// OrbVisualState.swift
///
/// Pure, UI-framework-free port of apps/mobile/src/AppModel.ts's `OrbVisualState` /
/// `OrbAnimationPreset` / `ORB_VISUAL_PRESETS`, plus the breathing-motion math from
/// apps/mobile/src/orb/motion.ts (`breathsPerMinuteToPeriodMs`, `pulseOpacityRange`,
/// `pulseScaleRange`, `resolvePulseMotionState`). No `import SwiftUI` / `import AppKit` here —
/// this is the model half; `OrbView.swift` in the `OrbMac` app target is the only consumer that
/// renders it.
///
/// Values are transcribed by hand from the RN source (not re-derived) so the macOS orb matches
/// the same visual spec instead of drifting toward an independent aesthetic choice.

/// The orb's 7 discrete visual states. Mirrors the TS union type
/// `'booting' | 'listening' | 'thinking' | 'working' | 'success' | 'paused' | 'error'` exactly.
public enum OrbVisualState: String, CaseIterable, Equatable, Sendable {
    case booting
    case listening
    case thinking
    case working
    case success
    case paused
    case error
}

/// One state's animation parameters. Field names and units match `OrbAnimationPreset` in
/// AppModel.ts: `speed` is an AuroraMist-internal drift/rotation multiplier (no physical unit),
/// `breathPeriodMs` is a full breath cycle in milliseconds, `breathDepth` is a 0-1 amplitude
/// fraction, `glow` is a 0-1 intensity, `colors` are exactly 3 hex strings (inner → mid → outer).
public struct OrbAnimationPreset: Equatable, Sendable {
    public let speed: Double
    public let breathPeriodMs: Double
    public let breathDepth: Double
    public let glow: Double
    public let colors: [String]

    public init(speed: Double, breathPeriodMs: Double, breathDepth: Double, glow: Double, colors: [String]) {
        precondition(colors.count == 3, "OrbAnimationPreset requires exactly 3 colors (inner/mid/outer)")
        self.speed = speed
        self.breathPeriodMs = breathPeriodMs
        self.breathDepth = breathDepth
        self.glow = glow
        self.colors = colors
    }

    /// Pure function: state -> preset. Mirrors `ORB_VISUAL_PRESETS[state]` field-for-field —
    /// see OrbVisualStateTests.swift for the acceptance test transcribing every one of the 7
    /// documented values from apps/mobile/src/AppModel.ts.
    public static func preset(for state: OrbVisualState) -> OrbAnimationPreset {
        allPresets[state]!
    }

    private static let allPresets: [OrbVisualState: OrbAnimationPreset] = [
        .booting: OrbAnimationPreset(
            speed: 3.5, breathPeriodMs: 5_500, breathDepth: 0.045, glow: 0.28,
            colors: ["#6576e8", "#aebcff", "#f5f7ff"]
        ),
        .listening: OrbAnimationPreset(
            speed: 5.5, breathPeriodMs: 4_200, breathDepth: 0.08, glow: 0.42,
            colors: ["#448bd6", "#8ed5ff", "#f2fbff"]
        ),
        .thinking: OrbAnimationPreset(
            speed: 7, breathPeriodMs: 3_900, breathDepth: 0.05, glow: 0.36,
            colors: ["#5c50e6", "#9892f5", "#e5e6ff"]
        ),
        .working: OrbAnimationPreset(
            speed: 4.6, breathPeriodMs: 6_500, breathDepth: 0.04, glow: 0.3,
            colors: ["#278f89", "#7ed2c7", "#effffb"]
        ),
        .success: OrbAnimationPreset(
            speed: 4, breathPeriodMs: 2_400, breathDepth: 0.1, glow: 0.55,
            colors: ["#58b89f", "#b5f1d5", "#fff9e9"]
        ),
        .paused: OrbAnimationPreset(
            speed: 1.3, breathPeriodMs: 7_500, breathDepth: 0.015, glow: 0.12,
            colors: ["#53627f", "#8997b5", "#d9e0ef"]
        ),
        // Warm degraded signal: noticeable without alarm-red strobing (matches motion.ts's own
        // rationale, transcribed from the AppModel.ts call site's comment).
        .error: OrbAnimationPreset(
            speed: 2.3, breathPeriodMs: 3_100, breathDepth: 0.055, glow: 0.3,
            colors: ["#c87552", "#f0b08a", "#fff0df"]
        ),
    ]
}

/// Breathing-motion math shared across every state, ported from orb/motion.ts. The task spec
/// pins this to "6 breaths/min (10000ms period) coherent-breathing pulse, opacity floor 0.5/depth
/// 0.25, scale depth 0.06" — independent of which of the 7 states is showing (each state's own
/// `breathPeriodMs`/`breathDepth` in `OrbAnimationPreset` governs the AuroraMist-style cloud drift
/// layer; `OrbBreathing` governs the separate low-level opacity/scale pulse envelope, matching the
/// TS split between `ORB_VISUAL_PRESETS` and `orb/motion.ts`'s `PULSE_*` constants).
public enum OrbBreathing {
    public static let opacityFloor = 0.5
    public static let opacityDepth = 0.25
    public static let scaleDepth = 0.06

    /// Ports `breathsPerMinuteToPeriodMs` from motion.ts.
    public static func breathsPerMinuteToPeriodMs(_ breathsPerMinute: Double) -> Double {
        60_000 / breathsPerMinute
    }

    /// The coherent-breathing period the spec calls out explicitly: 6 breaths/min == 10000ms.
    public static let coherentBreathPeriodMs: Double = breathsPerMinuteToPeriodMs(6)

    public struct PulseRange: Equatable, Sendable {
        public let low: Double
        public let high: Double
    }

    /// Ports `pulseOpacityRange` from motion.ts (floor-and-lift shape).
    public static func pulseOpacityRange() -> PulseRange {
        PulseRange(low: opacityFloor, high: opacityFloor + opacityDepth)
    }

    /// Ports `pulseScaleRange` from motion.ts.
    public static func pulseScaleRange() -> PulseRange {
        PulseRange(low: 1, high: 1 + scaleDepth)
    }

    /// Whether the breathing pulse should animate or freeze. Ports the reduce-motion half of
    /// `resolvePulseMotionState` from motion.ts — this file has no concept of "mic listening"
    /// (that's an RN-app-specific gate on a different layer), so it collapses to exactly the two
    /// cases the task spec asks for: animating, or frozen (held at the low end, zero oscillation)
    /// when the OS reduce-motion signal is on.
    public enum MotionState: Equatable, Sendable {
        case animating
        case frozen
    }

    public static func resolveMotionState(reduceMotionEnabled: Bool) -> MotionState {
        reduceMotionEnabled ? .frozen : .animating
    }
}
