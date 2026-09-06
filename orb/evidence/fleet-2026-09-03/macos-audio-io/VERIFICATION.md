# macOS audio I/O (mic capture + TTS playback) — evidence

Task: implement native macOS microphone capture (16kHz mono PCM16, 640-byte/320-sample
frames) and TTS playback (24kHz mono PCM16 streamed chunks, low-latency scheduling) in
`macos/OrbMac/Sources/OrbMacCore/`, using AVFoundation (AVAudioEngine + AVAudioConverter).
Reference algorithm: `ios/OrbMobile/OrbMic.swift` (accumulate-then-slice pattern).

Worktree: `company/products/adhd-focus-orb-worktrees/macos-audio-io`, branch
`feat/macos-audio-io`, based on `feat/macos-orb-scaffold` (M1 scaffold commit `54e9b6a`).

## Files added (all new, no overlap with the parallel protocol-client unit)

- `macos/OrbMac/Sources/OrbMacCore/AudioFormat.swift`
- `macos/OrbMac/Sources/OrbMacCore/PCM16FrameAccumulator.swift`
- `macos/OrbMac/Sources/OrbMacCore/PCM16Buffers.swift`
- `macos/OrbMac/Sources/OrbMacCore/AudioCapture.swift`
- `macos/OrbMac/Sources/OrbMacCore/AudioPlayback.swift`
- `macos/OrbMac/Tests/OrbMacCoreTests/PCM16FrameAccumulatorTests.swift`
- `macos/OrbMac/Tests/OrbMacCoreTests/PCM16BuffersTests.swift`
- `macos/OrbMac/Tests/OrbMacCoreTests/MicCaptureTests.swift`
- `macos/OrbMac/Tests/OrbMacCoreTests/TTSPlaybackTests.swift`

Nothing outside `macos/OrbMac/Sources/OrbMacCore/` and its test target was touched.
`macos/OrbMac/Sources/OrbMac/` (UI target) was not touched.

## TDD cycle (RED before GREEN)

All four test files were written first, referencing the not-yet-created
`MicCapture`/`TTSPlayback`/`PCM16Buffers`/`PCM16FrameAccumulator`/`AudioIOFormat` symbols,
then implemented. To leave a durable RED/GREEN artifact, the five implementation files
were also moved out of the target and `swift test` re-run to reproduce RED, then moved
back and re-run to reproduce GREEN. Full output: `red-green.txt`.

RED (implementation absent) — compiler errors, e.g. `cannot find 'TTSPlayback' in scope`,
`cannot find 'PCM16Buffers' in scope`, `cannot find 'AudioIOFormat' in scope`; `swift test`
exits non-zero.

GREEN (implementation restored): `Executed 16 tests, with 0 failures (0 unexpected)`,
`GREEN_EXIT=0`.

## Commands run, exit codes, reproduction from a fresh clone

From `macos/OrbMac/` on branch `feat/macos-audio-io`:

```sh
cd macos/OrbMac
swift build   # exit 0 — swift-build.txt
swift test    # exit 0, 16/16 passed — swift-test.txt
timeout 3 .build/debug/OrbMac; echo $?   # exit 124 (SIGTERM after 3s, no crash) — app-launch.txt
```

`swift build`: exit 0, `Build complete!` (see `swift-build.txt`).

`swift test`: exit 0. `MicCaptureTests` 2, `OrbMacCoreTests` 1 (pre-existing M1 smoke),
`PCM16BuffersTests` 5, `PCM16FrameAccumulatorTests` 5, `TTSPlaybackTests` 3 — 16 total, 0
failures (see `swift-test.txt`).

App launch smoke: exit 124 (healthy, per README convention) — see `app-launch.txt`.

## What is genuinely tested vs. an honest gap

**Tested, real:**
- `PCM16FrameAccumulator`: exact-multiple input, irregular chunk sizes across multiple
  `append()` calls, byte-layout (little-endian Int16), same-instance boundary case — all
  assert no drops/no duplicates/order preserved.
- `PCM16Buffers`: real `AVAudioConverter` runs against synthetic `AVAudioPCMBuffer`s —
  same-sample-rate full-scale Float32->Int16 identity conversion (deterministic, no
  resampling filter) and a real 48kHz->16kHz downsample of silence (asserts ~3:1
  sample-count ratio, zero output). Byte-packing/unpacking round-trips including a
  dropped-trailing-odd-byte edge case.
- `TTSPlayback.start()`/`scheduleChunk()`/`stop()`: run for real against this machine's
  actual default output device — `AVAudioEngine.start()` succeeded and `scheduleChunk()`
  was exercised with a real 100ms silent PCM16 chunk. Real AVFoundation engine/converter/
  render-graph wiring, not a mock (written to `XCTSkip` with an honest message if no
  usable output device exists in a given environment — on this machine it ran for real
  and passed).

**Honest gaps — NOT verified by this task:**
- **Microphone permission / real hardware capture**: `MicCapture.start()` against the
  actual system input device was NOT exercised. macOS requires an interactive TCC
  microphone-permission grant the first time a process taps `AVAudioEngine.inputNode`,
  and this session has no interactive UI to grant that prompt. `MicCaptureTests` only
  proves the `onFrame` closure wiring and that `stop()` is a safe no-op before `start()`
  — it does NOT prove real audio flows from a microphone through the converter and
  accumulator into `onFrame`. The conversion math and accumulator logic that would run
  on real audio are separately, fully unit-tested with synthetic buffers standing in for
  what a real tap callback would deliver.
- **Audible correctness of TTS playback**: nothing confirms by ear that scheduled audio
  sounds correct (no distortion, right pitch/speed) — only that the engine starts,
  accepts a buffer, and stops without error.
- **Real 24kHz TTS content**: `TTSPlaybackTests` uses silence as the test chunk
  (deterministic, sufficient to prove no crash/throw), not real speech-shaped PCM16.

## Registry / boundary notes

No registry change needed — new files inside the existing `OrbMacCore` unit per the M1
scaffold's documented module boundary (`macos/OrbMac/README.md`). `Package.swift` was not
modified — the existing `OrbMacCore` target already covers new files under
`Sources/OrbMacCore/` by SPM directory convention.
