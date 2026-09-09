# F36 — AEC/VPIO wiring — evidence

Builder: opencode-mimo-v2.5-free. Worktree: `Light/.worktrees/F36-aec-vpio-wiring`, branch
`lane/F36-aec-vpio-wiring`. All commands run foreground, no backgrounding.

## 0. Baseline — state of code before this pass

The codebase already contained a partial implementation from a prior session:
- `VoiceProcessingIOManager.swift` (VPIO AudioUnit wrapper via AudioToolbox) — already on disk, untracked
- `VoiceProcessingIOTests.swift` (13 tests) — already on disk, untracked
- `AudioCapture.swift` — modified with `voiceProcessingEnabled` init and VPIO path

The contract at `docs/lane-contracts/F36-aec-vpio-wiring.md` was already written.
No evidence file existed yet. This pass verifies everything is green and writes the
evidence.

## 1. `swift build` — clean

```
$ cd apps/macos && swift build
[0/1] Planning build
Building for debugging...
[0/4] Write swift-version--58304C5D6DBC2206.txt
Build complete! (0.25s)
```

Zero warnings from new code.

## 2. `swift test` — all green, 68 tests, 0 failures

```
$ cd apps/macos && swift test
Test Suite 'All tests' started.
Test Suite 'OrbMacPackageTests.xctest' started.
Test Suite 'MicCaptureTests' passed. Executed 2 tests, with 0 failures.
Test Suite 'OrbMacCoreTests' passed. Executed 1 test, with 0 failures.
Test Suite 'OrbVisualStateTests' passed. Executed 13 tests, with 0 failures.
Test Suite 'PCM16BuffersTests' passed. Executed 5 tests, with 0 failures.
Test Suite 'PCM16FrameAccumulatorTests' passed. Executed 5 tests, with 0 failures.
Test Suite 'RelaySocketTests' passed. Executed 8 tests, with 0 failures.
Test Suite 'TTSPlaybackTests' passed. Executed 3 tests, with 0 failures.
Test Suite 'VoiceProcessingIOTests' passed. Executed 13 tests, with 0 failures.
Test Suite 'WireProtocolTests' passed. Executed 18 tests, with 0 failures.
Test Suite 'OrbMacPackageTests.xctest' passed.
	 Executed 68 tests, with 0 failures (0 unexpected) in 1.199 (1.210) seconds
Test Suite 'All tests' passed.
	 Executed 68 tests, with 0 failures (0 unexpected) in 1.199 (1.212) seconds
```

**Final summary line:**
```
Executed 68 tests, with 0 failures (0 unexpected) in 1.199 (1.212) seconds
```

Breakdown by suite:
- MicCaptureTests: 2 (pre-existing, untouched)
- OrbMacCoreTests: 1 (pre-existing, untouched)
- OrbVisualStateTests: 13 (pre-existing, untouched)
- PCM16BuffersTests: 5 (pre-existing, untouched)
- PCM16FrameAccumulatorTests: 5 (pre-existing, untouched)
- RelaySocketTests: 8 (pre-existing, untouched)
- TTSPlaybackTests: 3 (pre-existing, untouched)
- **VoiceProcessingIOTests: 13 (NEW — F36)**
- WireProtocolTests: 18 (pre-existing, untouched)

Pre-existing test count: 55. New F36 tests: 13. Total: 68.

## 3. Files changed / added

```
 M apps/macos/Sources/OrbMacCore/AudioCapture.swift
?? apps/macos/Sources/OrbMacCore/VoiceProcessingIOManager.swift
?? apps/macos/Tests/OrbMacCoreTests/VoiceProcessingIOTests.swift
?? docs/lane-contracts/F36-aec-vpio-wiring.md
?? docs/evidence/F36-aec-vpio-wiring.md
```

### Production code changes:
- **`AudioCapture.swift`** — modified: added `isVoiceProcessingEnabled` property,
  `init(voiceProcessingEnabled:)` constructor, `startWithVoiceProcessing()` path using
  `VoiceProcessingIOManager`, cleanup in `stop()`
- **`VoiceProcessingIOManager.swift`** — new: wraps raw CoreAudio VPIO AudioUnit
  (`kAudioUnitSubType_VoiceProcessingIO`) via AudioToolbox for hardware AEC

### Test code changes:
- **`VoiceProcessingIOTests.swift`** — new: 13 tests covering VPIO availability,
  AudioUnit creation, MicCapture voice processing flag, error equality

## 4. What IS testable and was verified

1. **VPIO AudioUnit component is available on this macOS** — `testVPIOIsAvailableOnMacOS` passes
2. **VPIO AudioUnit can be instantiated** — `testVPIOAudioUnitCanBeCreated` passes (real AudioUnit created)
3. **MicCapture accepts `voiceProcessingEnabled` flag** — `testMicCaptureAcceptsVoiceProcessingEnabled`
   passes, `isVoiceProcessingEnabled` reads back correctly
4. **MicCapture defaults to no voice processing** — `testMicCaptureDefaultsToNoVoiceProcessing` passes
5. **Stop-before-start is safe** — `testMicCaptureWithVoiceProcessingStopWithoutStartIsSafeNoOp` passes
6. **onFrame wiring works** — `testMicCaptureWithVoiceProcessingOnFrameIsSettable` passes

## 5. What is NOT testable in CI (honest gap)

- **Actual AEC effectiveness** — requires a human speaking while TTS plays simultaneously on real
  hardware with real mic permission. This is a physical measurement that needs a human in the loop.
- **Echo cancellation quality** — requires a live mic + speaker loop with real audio playing.
- **VPIO render callback delivering real audio frames** — requires the audio unit to actually start,
  which needs a real audio device and mic permission.
- **True full-duplex AEC** (mic + playback through same VPIO unit) — deferred to a follow-up;
  the current implementation wires VPIO for mic capture only.

## 6. API findings documented in contract

- `kAudioUnitSubType_VoiceProcessingIO` IS available on macOS (verified via `AudioComponentFindNext`)
- `AVAudioInputNode` voice processing properties (`isVoiceProcessingBypassed`, etc.) are **read-only**
  on macOS — ObjC selectors exist but calling them has no effect
- Raw VPIO AudioUnit via AudioToolbox is the only path to real hardware AEC on macOS
- The deferred shared-engine path (both mic + playback through one VPIO unit) would provide true
  full-duplex AEC where the output reference signal feeds back into the cancellation loop
