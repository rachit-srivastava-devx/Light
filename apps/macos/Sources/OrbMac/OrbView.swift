import SwiftUI
import AppKit
import OrbMacCore

/// The orb's SwiftUI rendering, consuming `OrbVisualState`/`OrbAnimationPreset` from
/// `OrbMacCore`. This is the "AuroraMist" equivalent for macOS: the RN app builds a procedural
/// gradient cloud with Skia; here we layer SwiftUI `RadialGradient`/`AngularGradient` under a
/// `TimelineView` so the same organic drifting-cloud feel comes from real time-driven geometry,
/// not a static flat circle.
///
/// Honest scope note: this does not attempt pixel parity with AuroraMist's Skia shader — it is a
/// good-faith SwiftUI approximation of the same *properties* (breathing scale+opacity, slow
/// gradient rotation/drift, 3-stop color preset per state). Whether it actually *looks* organic
/// and calm is something only a human watching it render can judge; that verification is out of
/// reach for this task (see the task report's honest-gaps section).
public struct OrbView: View {
    public let state: OrbVisualState
    private let reduceMotionOverride: Bool?

    public init(state: OrbVisualState, reduceMotionOverride: Bool? = nil) {
        self.state = state
        self.reduceMotionOverride = reduceMotionOverride
    }

    /// NSWorkspace.shared.accessibilityDisplayShouldReduceMotion is the macOS analogue of RN's
    /// AccessibilityInfo.isReduceMotionEnabled(), per the task spec. `reduceMotionOverride` exists
    /// only so a preview/test harness can force either branch without depending on the live OS
    /// setting; production callers should leave it nil.
    private var reduceMotionEnabled: Bool {
        reduceMotionOverride ?? NSWorkspace.shared.accessibilityDisplayShouldReduceMotion
    }

    public var body: some View {
        let preset = OrbAnimationPreset.preset(for: state)
        let motionState = OrbBreathing.resolveMotionState(reduceMotionEnabled: reduceMotionEnabled)

        TimelineView(.animation(minimumInterval: 1.0 / 30.0, paused: false)) { timeline in
            let now = timeline.date.timeIntervalSinceReferenceDate

            // Breathing envelope: coherent 6-breaths/min pulse (10000ms period), scale/opacity
            // endpoints from OrbBreathing. Frozen (no oscillation, held at the low end) under
            // reduce-motion, per the task spec.
            let breathPhase: Double = {
                guard motionState == .animating else { return 0 }
                let periodSeconds = OrbBreathing.coherentBreathPeriodMs / 1000
                let t = now.truncatingRemainder(dividingBy: periodSeconds) / periodSeconds
                // (1 - cos)/2 gives a smooth 0->1->0 envelope with no discontinuity at the seam.
                return (1 - cos(2 * .pi * t)) / 2
            }()
            let opacityRange = OrbBreathing.pulseOpacityRange()
            let scaleRange = OrbBreathing.pulseScaleRange()
            let breathOpacity = opacityRange.low + (opacityRange.high - opacityRange.low) * breathPhase
            let breathScale = scaleRange.low + (scaleRange.high - scaleRange.low) * breathPhase

            // Slow gradient drift/rotation, scaled per-state by `preset.speed` (mirrors AuroraMist
            // receiving `speed` as a drift/rotation multiplier). Also frozen under reduce-motion.
            let driftAngle: Angle = {
                guard motionState == .animating else { return .degrees(0) }
                let radiansPerSecond = preset.speed * 0.05
                return .radians(now * radiansPerSecond)
            }()

            ZStack {
                // Outer glow halo — a soft radial wash sized by preset.glow.
                Circle()
                    .fill(
                        RadialGradient(
                            colors: [orbColor(preset.colors[1]).opacity(preset.glow), .clear],
                            center: .center,
                            startRadius: 0,
                            endRadius: 140
                        )
                    )
                    .frame(width: 280, height: 280)

                // The cloud body: an angular gradient (organic, non-concentric color banding)
                // rotating slowly, overlaid with a radial gradient for depth, both from the same
                // 3-stop preset palette (inner -> mid -> outer).
                Circle()
                    .fill(
                        AngularGradient(
                            colors: [
                                orbColor(preset.colors[0]),
                                orbColor(preset.colors[1]),
                                orbColor(preset.colors[2]),
                                orbColor(preset.colors[1]),
                                orbColor(preset.colors[0]),
                            ],
                            center: .center,
                            angle: driftAngle
                        )
                    )
                    .overlay(
                        Circle().fill(
                            RadialGradient(
                                colors: [orbColor(preset.colors[0]).opacity(0.55), .clear],
                                center: .center,
                                startRadius: 0,
                                endRadius: 90
                            )
                        )
                    )
                    .frame(width: 160, height: 160)
                    .scaleEffect(breathScale)
                    .opacity(breathOpacity)
            }
            .frame(width: 280, height: 280)
            .accessibilityLabel(Text("Focus orb, \(state.rawValue)"))
        }
    }
}

/// Parses a `#rrggbb` hex string (as used by the preset palettes) into a SwiftUI `Color`.
/// Malformed input falls back to a visible mid-gray rather than crashing — the preset table is
/// hand-authored, but this stays defensive at the render boundary.
private func orbColor(_ hex: String) -> Color {
    var s = hex
    if s.hasPrefix("#") { s.removeFirst() }
    guard s.count == 6, let value = UInt32(s, radix: 16) else {
        return Color(white: 0.5)
    }
    let r = Double((value >> 16) & 0xFF) / 255
    let g = Double((value >> 8) & 0xFF) / 255
    let b = Double(value & 0xFF) / 255
    return Color(red: r, green: g, blue: b)
}

#Preview("Booting") {
    OrbView(state: .booting)
}

#Preview("Listening") {
    OrbView(state: .listening)
}

#Preview("Reduce motion") {
    OrbView(state: .thinking, reduceMotionOverride: true)
}
