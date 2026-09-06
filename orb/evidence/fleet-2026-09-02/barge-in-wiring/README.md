# A8 barge-in wiring evidence

Date: 2026-09-02 22:02 IST
Branch: `fix/barge-in-wiring`
Base: `e942446a9e2e522b4046f8858c4534a5007ecbc2`

## Result

- Mic/VAD observations now reach a one-shot barge-in coordinator while the orb is speaking.
- At 200ms sustained post-processing speech, the coordinator synchronously calls the relay/provider cancellation port with the 100ms yield budget.
- The app then stops retained local playback, restores the bed voice gain, and resumes processing the user's mic stream.

## Evidence files

- `red.txt` — contract-first failure against the original implementation.
- `green.txt` — focused cancellation and retained-playback suite after the fix.
- `verify.txt` — final full-gate failure and byte-for-byte-equivalent baseline failure class.
- `runtime.txt` — relay cancellation integration test, realtime smoke, and bounded harness limitation.
- `FLEET-LEARNINGS-entry.md` — dated entry prepared because the requested shared file is absent outside this worktree.
- `sonnet-review.md` — mid-engineer (Sonnet) completion pass: independent line-by-line review of
  everything above, the one gap found and fixed (`App.tsx`'s `onSpeechStarting` was not setting
  `assistantSpeakingRef`), a fresh from-scratch re-verification of every claim in this directory
  (including a second, independent baseline isolation via a disposable detached worktree), and a
  full-suite before/after diff (616 → 618 passing tests, same 6 pre-existing broken suites, zero
  regressions).

## Remaining proof boundary

No physical-device AEC/speaker capture was run. The fake-T0 relay finishes synthesis before the delayed harness interrupt, so it cannot prove acoustic yield or live-provider cancellation. The deterministic mobile cancellation test and the relay's in-flight `SlowTts` integration test are green.

The new `AppVoiceWiring.test.tsx` integration test (added by codex) could not be executed in this
worktree — see `sonnet-review.md` for why (the `@pe/realtime-voice` module-resolution gap) — and was
verified by code trace only.
