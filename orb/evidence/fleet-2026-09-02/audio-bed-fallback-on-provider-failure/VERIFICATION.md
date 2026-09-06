# Defect A4 — Provider failure must fall back to the audio bed, not kill the session

**Date:** 2026-09-02
**Agent:** opencode (worktree `fix/audio-bed-fallback-on-provider-failure`, base `e942446`)
**Defect source:** `docs/REVIEW-2026-08-06-END-TO-END-FLOW.md` item 4
**Scope:** `backend/relay-rs/src/session.rs` (+ `protocol.rs`, `main.rs`)

## What the defect was

The relay's session state machine converted **any** STT or TTS provider failure into
`Action::SendAndClose(Closing { reason: ProviderFailure })`, transitioning the phase to **Ended**
and closing the WebSocket. Per the blueprint's audio-bed invariant (§5), provider failure must
preserve presence: the ambient bed must never gap, and the client must be told once ("I can't hear
you right now, but I'm still here") while the connection and presence contract continue. The old
behavior tore the session down instead.

## The fix

1. **`protocol.rs`** — added a `ServerFrame::AudioBedFallback { tenant_id, session_id }` frame the
   relay sends to tell the client to keep the bed alive and announce the outage once.
2. **`session.rs`**:
   - Added a `Degraded` phase (distinct from terminal `Ended`).
   - `fail()` now sets `Degraded` (not `Ended`) and returns `Action::SendFallback([AudioBedFallback])`
     instead of `SendAndClose`.
   - Added `Action::SendFallback(Vec<ServerFrame>)`.
   - While degraded, voice work (audio, speak, barge-in, end-of-turn) is ignored; **only `Pause`
     may close the connection** and end the session.
3. **`main.rs`**:
   - `execute()` handles `Action::SendFallback`: retires live speech, sends the fallback frame, but
     **keeps the socket open** (no `Outbound::Close`, no `connection_cancel.cancel()`), logging a
     `voice.degraded` event.
   - `describe_action` / `describe_frames` / `log_stt_transcripts` handle the new variants.
   - `close_after` remains `false` for `SendFallback`, so the read loop stays alive.

## Test evidence (before / after)

### RED — new test against OLD behavior

Temporarily restored the old `fail()` (set `Ended` + `SendAndClose`), kept the new test in place:

```
test session::tests::provider_failure_falls_back_to_audio_bed_and_keeps_session_alive ... FAILED

---- provider_failure_falls_back_to_audio_bed_and_keeps_session_alive stdout ----
session s1: provider unavailable: fake stt down
thread '...' panicked at src/session.rs:842:22:
expected SendFallback, got SendAndClose([Closing { tenant_id: "t1", session_id: "s1", reason: ProviderFailure }])

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured
```

### GREEN — with the fix, full cargo test

```
running 58 tests
...... <all ok>
test result: ok. 58 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Was 55 tests before; the fix adds 3 new/modified tests:
- `provider_failure_falls_back_to_audio_bed_and_keeps_session_alive`
- `tts_provider_failure_falls_back_to_audio_bed_and_keeps_session_alive`
- `degraded_session_ignores_voice_work_but_allows_pause_to_close`
- + `audio_bed_fallback_frame_round_trips_through_json` (protocol)

## Gate output

- `npm run lint` → `boundary-lint: clean` (exit 0) — presence plane still never imports voice bed.
- `npm run test:rs` → 58 passed, 0 failed (exit 0).
- `cargo test` (inside `backend/relay-rs`) → 58 passed, 0 failed.
- `npm run verify` → **does not reach the Rust step** because `npm run typecheck`
  (`tsc --noEmit -p apps/mobile`) fails **pre-existing** with `Cannot find module 'vitest' /
  'react' / 'react-native' / '@pe/realtime-voice'`. Verified this is pre-existing by stashing my
  changes and re-running: the identical mobile typecheck errors appear on the pristine base commit
  `e942446`. Root cause: this git worktree has no `node_modules` installed and the
  `file:../../../registry/...` dependencies do not resolve here. It is an environment gap, not a
  defect introduced by this change.

## What could NOT be verified

- The full `npm run verify` gate could not complete (blocked by the pre-existing mobile
  `node_modules`/registry-dependency gap described above). The Rust-relevant steps (`lint`,
  `test:rs`, `cargo test`) are green.
- No on-device audio was listened to; this is a deterministic unit/protocol fix. The audible
  "bed never gaps" claim requires a live device + failing/stopped provider sidecar, which was not
  exercised here.
