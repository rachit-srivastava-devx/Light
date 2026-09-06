# OrbMac

Native macOS SwiftUI host for the ADHD Focus Orb (unit **M1** — foundational scaffold).
This is a Swift Package Manager project, not an `.xcodeproj`, specifically so other
agents/units can build, test, and run it headlessly with `swift build` / `swift test` /
`swift run` without opening Xcode.

## Module boundaries (read this before adding code)

This package has two targets. Keep the boundary strict — it's what lets the protocol
client, audio, orb-visual, and orchestration units build on top of this scaffold in
parallel worktrees without stepping on each other.

### `OrbMacCore` (library target, `Sources/OrbMacCore/`)

**All non-UI logic.** No `import SwiftUI`, no `import AppKit`. This is where the
parallel units land their code:

- protocol client (talking to `relay-rs` / `relay-py`)
- audio capture/playback
- the focus-session state machine
- anything else that should be unit-testable without a window server

Because it's a separate target, it is independently testable via
`OrbMacCoreTests` — no app process, no UI, no simulator/window needed to run its
tests.

### `OrbMac` (executable target, `Sources/OrbMac/`)

**UI only.** The SwiftUI `@main App` struct (`OrbMacApp.swift`), scenes, and views.
It imports `OrbMacCore` and calls into it — it does not contain business logic
itself. The current `ContentView` is a placeholder (a plain colored circle); the
real orb visual rendering is a separate unit (**M-visual**) and should replace/extend
this view, not fork a second app target.

### `OrbMacCoreTests` (test target, `Tests/OrbMacCoreTests/`)

Tests for `OrbMacCore` only. Add real coverage here as the protocol client / audio /
state machine land. Currently contains one trivial smoke test
(`testModuleNameIsSet`) that exists only to prove the target builds, links, and is
importable — replace/extend, don't leave it as the only test once real logic exists.

## Requirements

- macOS 13+ (Package.swift platform minimum)
- Swift 6.3 / Xcode 26.6 toolchain (as installed in this repo's dev environment)

## Build / run / test

From `macos/OrbMac/`:

```sh
swift build          # compiles OrbMacCore + OrbMac
swift test           # runs OrbMacCoreTests
swift run OrbMac     # launches the app (opens a window)
```

To confirm the app starts without crashing in a non-interactive/CI context (no need
to screenshot):

```sh
timeout 3 .build/debug/OrbMac; echo $?
# exit 124 (SIGTERM from `timeout`, no crash before it) == healthy start
# any other signal/exit code before 3s elapses == investigate a crash
```

## What's NOT here yet

- Real protocol client, audio I/O, and state machine implementations
  (`OrbMacCore` currently has only a placeholder file) — these are separate units.
- Real orb visual rendering (`OrbMac`'s `ContentView` is a stand-in circle) — unit
  M-visual.
- Orchestration wiring all of the above together into a working focus session.

Each of those units should add files under the existing target directories above
rather than restructuring the package layout.
