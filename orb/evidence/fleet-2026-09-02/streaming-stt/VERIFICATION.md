# A5 streaming STT verification

Date: 2026-09-02 21:55 IST
Branch: `fix/streaming-stt`
Base/HEAD before commit: `e942446a9e2e522b4046f8858c4534a5007ecbc2`

## Scope and registry decision

`build-new` inside the existing product-owned provider adapters. The registry's
`voice-realtime` service contains the T0 contract/memory adapter, not a scaled Sarvam or Deepgram
partial-transcription implementation. No dependency or provider door was added.

Sarvam and Deepgram now submit one bounded 1.0-second PCM prefix during `pushAudio`, return the
provider transcript as `{is_final:false}`, retain the complete per-session buffer, and perform the
authoritative full transcription at `endTurn`. The one-prefix limit caps duplicated provider audio
at one second per multi-second turn.

## Test-first evidence

RED command:

```text
npx vitest run backend/voice-provider-sidecar/__tests__/real-providers.test.ts
```

Exit `1`: 2 failed / 18 passed. Both Sarvam and Deepgram reported `partialIndex = -1` while replaying
the checked-in 6.66-second `long-task.wav` in 100 ms PCM frames. Raw output: `focused-red.log`.

GREEN command: same command after implementation.

Exit `0`: 20 passed / 0 failed. Raw output: `focused-green.log`.

## Sidecar verification

```text
npx vitest run backend/voice-provider-sidecar
npx tsc --noEmit -p backend/voice-provider-sidecar
```

- Sidecar suite exit `0`: 7 files, 60 tests passed.
- Sidecar typecheck exit `0`.
- Raw output: `sidecar-tests.log`, `sidecar-typecheck.log`.

## Root verify and baseline isolation

```text
npm run verify
```

Final exit `2`. Boundary lint passed, then mobile typecheck stopped the gate with 20 errors involving
`VoiceFeatureInput.tenant_id/session_id` and unresolved `@pe/voice-realtime` imports.

Isolation procedure:

1. Saved the diff for only the six A5 source/test files.
2. Reverse-applied that diff, leaving exact `e942446` source in this worktree.
3. Re-ran `npm run verify` and restored the A5 diff.
4. Compared every `error TS` line between baseline and final runs.

Baseline exit was also `2`; the baseline and final 20-line failure sets were identical (`diff` exit
`0`). Raw outputs: `verify-baseline-e942446.log`, `verify-final.log`,
`verify-failure-diff.log`.

## Not verified

- No Sarvam or Deepgram API key was available, so provider network behavior, real transcript text,
  billed duration, and latency percentiles were not measured.
- This change verifies app-visible partial emission through the existing HTTP sidecar contract. It
  does not claim a persistent provider WebSocket session or voice-to-voice latency-gate proof.
