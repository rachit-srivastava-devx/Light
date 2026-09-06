# macos-orb-visual — evidence report

Task: implement the orb's visual-state model (OrbMacCore, UI-framework-free) and its SwiftUI
rendering (OrbMac), matching apps/mobile/src/AppModel.ts's `ORB_VISUAL_PRESETS` and
apps/mobile/src/orb/motion.ts exactly. Worktree:
`company/products/adhd-focus-orb-worktrees/macos-orb-visual`, branch `feat/macos-orb-visual`
(based on `feat/macos-orb-scaffold`), base commit `54e9b6aa59046b4619a71568060268efccda88ef`.

## Files added / changed

- `macos/OrbMac/Sources/OrbMacCore/OrbVisualState.swift` (new) — `OrbVisualState` enum (7 cases),
  `OrbAnimationPreset` struct + `OrbAnimationPreset.preset(for:)` pure function, `OrbBreathing`
  enum (breath period/opacity/scale math + reduce-motion resolution). No `import SwiftUI` /
  `import AppKit`.
- `macos/OrbMac/Tests/OrbMacCoreTests/OrbVisualStateTests.swift` (new) — 13 tests: one per state
  (7), an exhaustiveness check, and 5 for the breathing-motion helpers.
- `macos/OrbMac/Sources/OrbMac/OrbView.swift` (new) — SwiftUI view consuming `OrbVisualState`,
  `TimelineView`-driven breathing (scale+opacity) and slow gradient rotation/drift.
- `macos/OrbMac/Sources/OrbMac/OrbMacApp.swift` (modified) — `ContentView` now hosts `OrbView`
  bound to `@State private var orbState: OrbVisualState = .booting`, replacing the placeholder
  colored circle.

## Swift API designed

```swift
public enum OrbVisualState: String, CaseIterable, Equatable, Sendable {
    case booting, listening, thinking, working, success, paused, error
}

public struct OrbAnimationPreset: Equatable, Sendable {
    public let speed: Double
    public let breathPeriodMs: Double
    public let breathDepth: Double
    public let glow: Double
    public let colors: [String]          // exactly 3 hex strings: inner, mid, outer
    public static func preset(for state: OrbVisualState) -> OrbAnimationPreset
}

public enum OrbBreathing {
    public static let opacityFloor = 0.5
    public static let opacityDepth = 0.25
    public static let scaleDepth = 0.06
    public static func breathsPerMinuteToPeriodMs(_ bpm: Double) -> Double
    public static let coherentBreathPeriodMs: Double   // = 10_000 (6 bpm)
    public struct PulseRange: Equatable, Sendable { let low: Double; let high: Double }
    public static func pulseOpacityRange() -> PulseRange   // 0.5 ... 0.75
    public static func pulseScaleRange() -> PulseRange     // 1.0 ... 1.06
    public enum MotionState: Equatable, Sendable { case animating, frozen }
    public static func resolveMotionState(reduceMotionEnabled: Bool) -> MotionState
}

// OrbMac (UI target):
public struct OrbView: View {
    public init(state: OrbVisualState, reduceMotionOverride: Bool? = nil)
}
```

All 7 preset values transcribed field-for-field from `ORB_VISUAL_PRESETS` in
`apps/mobile/src/AppModel.ts` (lines 98-149) and the breathing constants from
`apps/mobile/src/orb/motion.ts` (`BREATH_PULSE_PERIOD_MS` = 6bpm/10000ms,
`PULSE_OPACITY_FLOOR`/`PULSE_OPACITY_DEPTH` = 0.5/0.25, `PULSE_SCALE_DEPTH` = 0.06,
`resolvePulseMotionState`'s reduce-motion branch).

## Test-first sequence

### RED (module doesn't exist yet)

Command: `swift test` in `macos/OrbMac`, run against the tree with only
`OrbVisualStateTests.swift` added (no `OrbVisualState.swift` yet).

```
error: cannot find 'OrbAnimationPreset' in scope
error: cannot find 'OrbBreathing' in scope
error: type 'Equatable' has no member 'animating'
error: type 'Equatable' has no member 'frozen'
error: fatalError
```

Full failing compiler output is reproducible by removing the two new `Sources/` files (leaving
only the test file) and rerunning `swift test`.

### GREEN (after implementing OrbVisualState.swift)

Command: `swift test` — see `swift-test.log` in this directory for the full pasted transcript.
Summary: `Executed 14 tests, with 0 failures (0 unexpected)` (1 pre-existing smoke test +
13 new: 7 per-state preset checks, 1 exhaustiveness check, 5 breathing-motion checks).
Exit code: 0.

## Build / launch verification

```
$ cd macos/OrbMac && swift build
Build complete! (0.63s)
BUILD_EXIT:0

$ swift test
... (see swift-test.log)
TEST_EXIT:0

$ timeout 3 .build/debug/OrbMac; echo "LAUNCH_EXIT:$?"
LAUNCH_EXIT:124
```

Exit 124 == SIGTERM from `timeout` after 3s with no earlier crash/signal — the same "healthy
start" pattern the M1 scaffold unit's README documents. Confirms the app process starts and stays
up with the real `OrbView` wired into `ContentView`, not just the placeholder circle.

Full logs: `swift-build.log`, `swift-test.log` (this directory).

## Reproduce from a fresh clone

```sh
git clone <repo> && cd <repo>
git checkout feat/macos-orb-visual   # or fetch this worktree's branch
cd macos/OrbMac
swift build          # expect exit 0
swift test           # expect exit 0, 14 tests passed
timeout 3 .build/debug/OrbMac; echo $?   # expect 124
```

No external dependencies beyond the Swift 6.3 / Xcode 26.6 toolchain the README already states;
`Package.swift` was not modified (no new SPM dependencies added).

## Honest gaps

- **Cannot verify the animation looks organic/correct headlessly.** `swift test` and the launch
  smoke test only prove: (a) the 7 presets carry the exact documented numbers, (b) the app process
  builds and doesn't crash on start with `OrbView` mounted. Neither proves the rendered result
  reads as a "drifting organic cloud" rather than, say, a jarring or barely-visible animation —
  that requires a human actually watching `swift run OrbMac` render, which this task cannot do.
  I did not claim visual fidelity to AuroraMist (the RN/Skia implementation) anywhere in the code
  comments or this report — `OrbView.swift`'s own doc comment says so explicitly.
- The view does not yet subscribe to `NSWorkspace`'s live reduce-motion change notifications
  (`NSWorkspace.accessibilityDisplayOptionsDidChangeNotification`) — it reads the value once per
  render pass via `TimelineView`'s own redraw cadence, which is adequate for a 30fps timeline but
  not tested against a live toggle of the OS setting mid-session (no such test exists; would need
  a UI test harness this unit doesn't have). `reduceMotionOverride` exists specifically so a
  future test/preview could inject either branch without touching the live OS setting.
- Driving `orbState` from the real protocol client is a later integration unit, explicitly out of
  scope per the task brief ("real state-driving from the protocol client is a later integration
  unit, not yours").
- `Package.swift` untouched — no new SPM dependency was needed (pure SwiftUI/Foundation/AppKit).
