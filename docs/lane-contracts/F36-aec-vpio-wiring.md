# F36 — AEC/VPIO Wiring Contract

**Feature:** AEC/VPIO wiring for the macOS app `apps/macos`
**Repo:** `apps/macos` (OrbMac / OrbMacCore)
**Date:** 2026-09-07

---

## Task

Wire CoreAudio Voice-Processing I/O (hardware AEC) into the macOS client so that when the orb is speaking (TTS playback), the mic does not pick up the orb's own voice. Without this, barge-in false-triggers and echo-back into the transcript.

Currently: zero hits for `voiceProcessing` / `VoiceProcessingIO` / `setVoiceProcessingEnabled` in `Sources/`.

## Ground-Truth API Findings (verified on macOS SDK 26.5)

1. **`kAudioUnitSubType_VoiceProcessingIO` IS available on macOS** — verified via `AudioComponentFindNext` with `kAudioUnitSubType_VoiceProcessingIO`. The component can be instantiated and an `AudioUnit` instance created.

2. **`AVAudioInputNode` voice processing properties are READ-ONLY on macOS** — the ObjC selectors `setVoiceProcessingBypassed:`, `setVoiceProcessingAGCEnabled:`, `setVoiceProcessingInputMuted:` exist at runtime, but calling them via `perform(_:with:)` does not change the property values. `isVoiceProcessingBypassed` starts as `false` and cannot be toggled. This means `AVAudioEngine`'s convenience API does NOT provide a way to enable/configure voice processing on macOS.

3. **The raw VPIO AudioUnit** must be used directly via AudioToolbox for real AEC on macOS.

## Killed Alternatives (≥2)

### Alternative 1: AVAudioEngine.setVoiceProcessingEnabled / inputNode.isVoiceProcessingBypassed
- **What:** Use `AVAudioEngine`'s built-in voice processing API (like iOS's `enableVoiceProcessing()`)
- **Why killed:** On macOS, `AVAudioInputNode`'s voice processing properties are read-only. Setting them via ObjC selector has no effect. The convenience API does not provide a toggle.
- **Killed with evidence:** `isVoiceProcessingBypassed` remains `false` after calling `setVoiceProcessingBypassed:` via ObjC runtime.

### Alternative 2: Software-only AEC (e.g. SpeexDSP / WebRTC APM)
- **What:** Use a cross-platform software AEC library to cancel echo in the captured audio
- **Why killed:** Adds a large dependency, duplicates work CoreAudio already does in hardware, and the VPIO AudioUnit is verified available on macOS — there is no reason to use a software fallback when the OS provides hardware AEC natively.

### Alternative 3 (not killed but deferred): Shared-engine VPIO (both mic + playback through one AVAudioEngine)
- **What:** Re-architect `MicCapture` and `TTSPlayback` to share a single `AVAudioEngine` instance with VPIO as the underlying audio unit
- **Why deferred:** Requires significant restructuring of both classes. The current implementation wires VPIO for mic capture only. True AEC (where the output reference signal feeds back into the VPIO) requires both paths on the same engine — this is acknowledged as the honest gap and left for a follow-up.

## Interface Change

### New: `VoiceProcessingIOManager` (AudioToolbox-based)

```swift
/// Wraps a raw CoreAudio VoiceProcessingIO AudioUnit for mic capture.
/// On macOS, AVAudioEngine's voice processing properties are read-only,
/// so we must use the VPIO AudioUnit directly.
public final class VoiceProcessingIOManager {
    public static var isAvailable: Bool { get }  // AudioComponentFindNext check
    public init() throws                          // creates VPIO AudioUnit
    public func start() throws                    // starts the audio unit
    public func stop()                            // stops and disposes
    public var onProcessedFrame: ((Data) -> Void)? // callback for processed mic frames
}
```

### Modified: `MicCapture`

```swift
// Existing init unchanged:
public init()

// New:
public init(voiceProcessingEnabled: Bool)

// New property:
public private(set) var isVoiceProcessingEnabled: Bool
```

When `voiceProcessingEnabled: true` and `VoiceProcessingIOManager.isAvailable`, the capture path uses the VPIO AudioUnit instead of a plain `AVAudioEngine.inputNode`.

### No change: `TTSPlayback`
Playback path is unchanged. For true AEC, both mic and playback need the same VPIO unit — this is the deferred work.

## Acceptance Test

### Test 1: VPIO unit creation
```swift
func testVPIOAudioUnitCanBeCreated() throws {
    guard VoiceProcessingIOManager.isAvailable else {
        throw XCTSkip("VPIO not available on this platform")
    }
    let manager = try VoiceProcessingIOManager()
    // manager created successfully — proves the AudioUnit instantiated
}
```

### Test 2: MicCapture with voiceProcessingEnabled
```swift
func testMicCaptureAcceptsVoiceProcessingFlag() {
    let capture = MicCapture(voiceProcessingEnabled: true)
    XCTAssertTrue(capture.isVoiceProcessingEnabled)
    
    let capture2 = MicCapture(voiceProcessingEnabled: false)
    XCTAssertFalse(capture2.isVoiceProcessingEnabled)
}
```

### Test 3: MicCapture voiceProcessingEnabled defaults to false
```swift
func testMicCaptureDefaultsToNoVoiceProcessing() {
    let capture = MicCapture()
    XCTAssertFalse(capture.isVoiceProcessingEnabled)
}
```

### What IS NOT testable in CI (honest gap):
- Actual AEC effectiveness (requires a human speaking while TTS plays simultaneously on real hardware)
- True echo cancellation quality (requires a live mic + speaker loop with real audio)
- The deferred shared-engine VPIO path (both mic + playback through one unit)

## Done Definition

1. `swift build` clean — zero warnings from new code
2. `swift test` green — all 55 existing tests + new tests pass
3. Written evidence at `docs/evidence/F36-aec-vpio-wiring.md` with real pasted command output
4. Dated entry appended to `/Users/rachitsrivastava/youtube/Principal Engineering/FLEET-LEARNINGS.md`
5. Status file updated at the absolute path, render.sh run
6. Committed to `lane/F36-aec-vpio-wiring` — NOT merged to master
