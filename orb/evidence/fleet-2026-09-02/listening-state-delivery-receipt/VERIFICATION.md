# A2 — listening-state-delivery-receipt

Worktree: `company/products/adhd-focus-orb-worktrees/listening-state-delivery-receipt`
Branch: `fix/listening-state-delivery-receipt`, base `e942446`.

## Defect

The orb's "listening" UI state (`orbVisualStateFromEnvelope` in `apps/mobile/src/AppModel.ts`)
flipped to `'listening'` purely from `micStatus === 'active'` — a signal that only means the local
mic hardware opened and produced its first frame (`App.tsx`'s `NativeMicPort` wiring). Nothing in
the client ever confirmed that `backend/relay-rs` actually accepted the `start_listening` frame and
opened the STT stream: `Session::handle_client_frame`'s `StartListening` arm in
`backend/relay-rs/src/session.rs` returned `Action::Send(vec![])` — no reply at all. A user could
see "listening" against a relay that never received the frame, rejected it (identity mismatch), or
whose socket died immediately after.

## Fix

1. `backend/relay-rs/src/protocol.rs` — new `ServerFrame::ListeningConfirmed { tenant_id,
   session_id }` variant: the delivery receipt.
2. `backend/relay-rs/src/session.rs` — `StartListening`'s success arm now returns
   `Action::Send(vec![ServerFrame::ListeningConfirmed { .. }])` instead of an empty send. A rejected
   `StartListening` (identity mismatch) still returns `Action::Ignore` — no receipt for a frame the
   relay dropped.
3. `backend/relay-rs/src/main.rs` — `describe_frames` (dev-logs) gets a `ListeningConfirmed` arm so
   the match stays exhaustive.
4. `apps/mobile/src/voice/contracts.ts` — mirrors the new `listening_confirmed` `ServerFrame`
   variant on the client side of the wire protocol.
5. `apps/mobile/src/voice/RelayClient.ts` — new optional `onListeningConfirmed` handler on
   `RelayHandlers`; `receive()` dispatches `listening_confirmed` frames to it.
6. `apps/mobile/src/App.tsx` — new `relayListeningConfirmed` state, set `true` by
   `onListeningConfirmed`, reset to `false` in `onClosing` (a closed/failed relay is not listening to
   anything, receipt or not). Fed into `screenModelFromEnvelope` as the new `listeningConfirmed`
   lifecycle signal.
7. `apps/mobile/src/AppModel.ts` — `OrbLifecycleSignals` gains `listeningConfirmed?: boolean`
   (default `false`). `orbVisualStateFromEnvelope`'s `INTAKE`/`CLARIFY` branch now requires
   `micStatus === 'active' && listeningConfirmed` to return `'listening'`; local mic-open alone falls
   back to `'booting'`.

Tests added (this repo has no `tests/large/acceptance/**` directory; new/extended tests written
per the SOW brief, in files I own):
- `backend/relay-rs/src/session.rs`: `start_listening_sends_a_listening_confirmed_delivery_receipt`,
  `a_rejected_start_listening_sends_no_delivery_receipt`.
- `apps/mobile/src/voice/voice.test.ts`: `dispatches the relay delivery receipt separately from
  local mic-open`.
- `apps/mobile/src/AppModel.test.ts`: `does not show "listening" on local mic-open alone, only on
  the relay delivery receipt`.

## RED confirmed (real output, saved alongside this file)

`cargo-RED-before-fix.log` — with `StartListening`'s arm reverted to `Action::Send(vec![])`:
```
thread '...start_listening_sends_a_listening_confirmed_delivery_receipt' panicked ...
assertion `left == right` failed
  left: []
 right: [ListeningConfirmed { tenant_id: "t1", session_id: "s1" }]
test result: FAILED. 0 passed; 1 failed
```

`vitest-RED-before-fix.log` — with `AppModel.ts`'s gate reverted to `micStatus === 'active' ?
'listening' : 'booting'` (dropping the `listeningConfirmed` check):
```
× does not show "listening" on local mic-open alone, only on the relay delivery receipt
AssertionError: expected 'listening' to be 'booting'
Tests  1 failed | 10 passed (11)
```

## GREEN confirmed (real output)

- `cargo-test.log` (`npm run test:rs`): `test result: ok. 57 passed; 0 failed; 0 ignored; 0
  measured; 0 filtered out` (was 55 before my 2 new tests).
- `apps/mobile/src/AppModel.test.ts` + `apps/mobile/src/voice/voice.test.ts` focused run: `Test
  Files 2 passed (2)`, `Tests 49 passed (49)`.
- `pytest.log` (`npm run test:py`): `393 passed, 3 warnings`.

## Full `npm run verify`

`npm run verify` is `lint && typecheck && test && test:py && test:rs`. In this worktree,
`typecheck`/`test` fail on things that are **not this diff**:

1. **A genuine worktree-depth bug, already documented by a prior fleet worker**
   (`FLEET-LEARNINGS.md`, `ws-reconnect-timeout` entry): `apps/mobile/tsconfig.json`,
   `backend/gateway-sidecar/tsconfig.json`, and `vitest.config.ts` all resolve `@pe/voice-realtime` /
   `@pe/llm-gateway` via a fixed count of `../` segments computed for the *sibling* checkout's depth
   (`company/products/adhd-focus-orb/...`). This worktree sits one level deeper
   (`company/products/adhd-focus-orb-worktrees/listening-state-delivery-receipt/...`), so every one
   of those relative paths resolves one directory short and `@pe/voice-realtime`/`@pe/llm-gateway`
   report as missing modules — see `vitest-initial-with-depth-bug.log`. Confirmed by running
   `npm run typecheck` in the sibling checkout `company/products/adhd-focus-orb` (unmodified,
   untouched by this task): **exit 0, zero errors** — the identical `tsconfig.json`/`vitest.config.ts`
   content is only broken by the worktree's extra path segment, not by anything in the codebase. I
   temporarily added one more `../` to the three files, ran the gate, then reverted all three before
   committing (`git status --short` shows only my intended 9 files afterward). With the depth
   corrected: `npx tsc --noEmit -p apps/mobile` → exit 0 (my owned area, fully clean); `npx tsc
   --noEmit -p backend/voice-provider-sidecar` → exit 0; vitest module-resolution errors disappear
   entirely (see `vitest-depth-fixed-full.log`, 693-694/694 passing depending on run).
2. **One real, pre-existing, out-of-scope gap** unrelated to (1): `backend/gateway-sidecar/src/index.ts`
   imports `@pe/llm-gateway/adapters/gemini`, but neither `tsconfig.json`'s `paths` nor
   `vitest.config.ts`'s `alias` list has ever had a mapping for it (only `memory` and `anthropic` are
   mapped) — `backend/gateway-sidecar/tsconfig.json:20-25`. This is committed code at base `e942446`,
   in a file this task never touches (`backend/gateway-sidecar` is A1/A9's area per the SOW, not
   A2's). It blocks the `typecheck` step of the `&&`-chained `verify` script regardless of depth. Two
   of `backend/gateway-sidecar`'s vitest suites (`adapterTelemetry.test.ts`, `complete.test.ts`) fail
   the same way for the same reason.
3. **Two flaky, load-sensitive perf-budget tests**, neither touching any file in this diff:
   `apps/mobile/src/voice/NativeMicPort.test.ts` (`bounds analysis cost on a huge frame`, budget
   50ms) and `apps/mobile/src/presence/NoiseEngine.test.ts` (`never exceeds the [-1, 1] clamp`, 5000ms
   timeout). Both failed intermittently under the concurrent multi-agent load on this machine (other
   fleet workers building/testing in parallel worktrees) and both pass cleanly in isolation —
   `NoiseEngine.test.ts` alone: `20 tests | 20 passed`, the failing case at 2225ms against its
   internal budget.

None of (1)-(3) are caused by this diff — `git diff --stat` touches exactly 9 files, none of which is
`backend/gateway-sidecar/**`, `NativeMicPort.ts`, or `NoiseEngine.ts`:

```
 apps/mobile/src/App.tsx              | 17 ++++++++++++-
 apps/mobile/src/AppModel.test.ts     | 27 +++++++++++++++++++-
 apps/mobile/src/AppModel.ts          | 14 ++++++++++-
 apps/mobile/src/voice/RelayClient.ts |  8 ++++++
 apps/mobile/src/voice/contracts.ts   |  8 ++++++
 apps/mobile/src/voice/voice.test.ts  | 16 ++++++++++++
 backend/relay-rs/src/main.rs         |  1 +
 backend/relay-rs/src/protocol.rs     |  8 ++++++
 backend/relay-rs/src/session.rs      | 48 +++++++++++++++++++++++++++++++++---
 9 files changed, 141 insertions(+), 6 deletions(-)
```

Isolated verdicts, each run to completion with the real command:
- `npm run lint` → `boundary-lint: clean` (exit 0).
- `npx tsc --noEmit -p apps/mobile` (depth-corrected) → exit 0.
- `npx tsc --noEmit -p backend/voice-provider-sidecar` (depth-corrected) → exit 0.
- `npm run test` (vitest, depth-corrected) → 693-694/694 (the 1 flake alternates between two
  unrelated perf tests depending on machine load; isolated re-runs of each are green).
- `npm run test:py` → 393/393.
- `npm run test:rs` → 57/57.

## Manual drive / reasoning through the fixed path

Traced the runtime call path end to end rather than trusting the green tests alone:

1. `App.tsx` mounts, opens the WebSocket, constructs `RelayClient` with the new
   `onListeningConfirmed: () => setRelayListeningConfirmed(true)` handler wired in alongside the
   existing `onTranscript`/`onClosing`/etc. (`apps/mobile/src/App.tsx:391`).
2. The mic-permission flow calls `relay.startListening()` → `RelayClient.startListening()` → sends
   `{"type":"start_listening", tenant_id, session_id}` over the wire
   (`apps/mobile/src/voice/RelayClient.ts:171-178`, unchanged).
3. `backend/relay-rs`'s socket loop calls `Session::handle_client_frame(ClientFrame::StartListening
   {..})`. With identity fields present and no mismatch, this now returns
   `Action::Send(vec![ServerFrame::ListeningConfirmed { tenant_id, session_id }])`
   (`backend/relay-rs/src/session.rs:279-306`) instead of the old empty send.
4. The socket layer serialises that `Action::Send` payload onto the wire (unchanged outbound
   plumbing in `main.rs`) as `{"type":"listening_confirmed", tenant_id, session_id}`.
5. Back on the client, `RelayClient.receive()` parses the frame, matches
   `case 'listening_confirmed'`, logs `relay_client.listening_confirmed`, and calls
   `handlers.onListeningConfirmed?.()` (`apps/mobile/src/voice/RelayClient.ts:311-315`) → App.tsx's
   `setRelayListeningConfirmed(true)`.
6. `screenModelFromEnvelope`/`orbVisualStateFromEnvelope` now receives `listeningConfirmed: true` in
   its lifecycle signals and, for `INTAKE`/`CLARIFY` with `micStatus === 'active'`, returns
   `'listening'` — the orb only shows "listening" once this full round trip has completed, not on
   local mic-open alone.
7. Regression path: if the relay never accepts the session (identity mismatch, dead socket before
   the receipt arrives, or a `Closing` frame arriving instead), `relayListeningConfirmed`
   stays/goes back to `false` (the `onClosing` handler resets it), and
   `orbVisualStateFromEnvelope` falls back to `'booting'` even with the mic already capturing —
   exactly the case this task exists to fix.

No live device/simulator was available in this sandboxed environment to record actual
voice-to-voice audio evidence (the app's own bar #6 asks for that for audible claims) — this is
deterministic UI-state wiring, verified by tracing the actual runtime call path above plus the
unit/integration tests on both sides of the wire protocol, not a claim about audio quality or
latency. Flagged here as the one unverified claim category.

## Reproduce from a fresh clone

```bash
git clone <repo> && cd <repo>
git worktree add ../adhd-focus-orb-worktrees/listening-state-delivery-receipt \
  -b fix/listening-state-delivery-receipt e942446
cd ../adhd-focus-orb-worktrees/listening-state-delivery-receipt

# Needs a sibling `adhd-focus-orb` checkout with `npm install` already run (workspace deps are
# `file:` links resolved relative to the canonical, non-worktree path):
ln -s ../../adhd-focus-orb/node_modules node_modules
ln -s ../../../../adhd-focus-orb/backend/relay-py/.venv backend/relay-py/.venv

npm run test:rs   # backend/relay-rs — 57/57
npm run test:py   # backend/relay-py — 393/393
npx vitest run apps/mobile/src/AppModel.test.ts apps/mobile/src/voice/voice.test.ts   # 49/49
```

Full `npm run verify`/`npm run test` need the one-`../`-deeper correction to
`apps/mobile/tsconfig.json`, `backend/gateway-sidecar/tsconfig.json`, and `vitest.config.ts`
described above (temporary, not committed) to get past the worktree-depth artifact — the
`backend/gateway-sidecar` `@pe/llm-gateway/adapters/gemini` gap and the two perf-budget flakes are
real but out of this task's scope.
