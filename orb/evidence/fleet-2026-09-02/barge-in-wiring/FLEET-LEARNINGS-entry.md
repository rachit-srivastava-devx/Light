### 2026-09-02-2202 — codex — barge-in-wiring

- Worked: keep sustained-speech timing deterministic in `BargeIn.ts`, inject the provider-cancel effect, and make `VoiceLoopController` the transport owner; the app owns only retained playback/bed restoration.
- Proof: the contract-first test changed from 1 failed/27 passed to 29/29 passed; relay in-flight cancellation is 1/1 green; `smoke:realtime` is green.
- Gotcha: `../../FLEET-LEARNINGS.md` does not exist from this worktree or anywhere under the bounded `company/` tree, and creating it would violate the instruction to work only in this directory. This entry is preserved here for the fleet owner to append.
- Baseline red: `npm run verify` exits 2 on the same missing `@pe/realtime-voice` exports in both this worktree and an unmodified `e942446` archive.
