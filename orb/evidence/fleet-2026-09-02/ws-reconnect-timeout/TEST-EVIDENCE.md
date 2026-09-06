# A3 WebSocket reconnect/open-timeout evidence

Date: 2026-09-02
Branch: `fix/ws-reconnect-timeout`
Base: `e942446a9e2e522b4046f8858c4534a5007ecbc2`

## Test-first RED

Command:

```text
npm test -- apps/mobile/src/voice/RelaySocket.test.ts
```

Result: exit 1; 8 tests ran, 6 passed, 2 failed. The new timeout-subtype and one-reconnect tests
both failed because `RelayOpenTimeoutError` and `RelayUnexpectedCloseError` did not exist in the
baseline. Before dependencies were installed, the same command exited 127 with `vitest: command
not found`; that was an isolated-worktree setup failure, not the defect RED.

## Focused GREEN

Command:

```text
npm run typecheck && npm test -- apps/mobile/src/voice/RelaySocket.test.ts
```

Result: exit 0; typecheck passed; 9/9 focused tests passed. Deterministic fake-timer assertions
proved a 50 ms backoff, zero attempts at 49 ms, exactly one attempt at 50 ms, and no second attempt
after the replacement socket dropped. The RelayClient test also proved `start_listening` is sent
before the audio frame queued during reconnect.

## App lifecycle verification

Result: 52/52 targeted RelaySocket, RelayClient, and AppSurface tests passed. The app-level test
proved mic capture remains active during the one retry and is released after the retry is exhausted.

## Full gate

Command: `npm run verify`

Result: exit 0. See `npm-run-verify.log` for the complete output: boundary-lint clean, TypeScript
typecheck passed, 694/694 Vitest tests passed, 393/393 pytest tests passed (3 warnings), and 58/58
Rust tests passed.

## Invocation gotcha and manual limits

This worktree is one directory deeper than the canonical product checkout. Its checked-in `file:`
dependency and registry aliases therefore resolve one level too shallow. Verification used
worktree-local dependency copies plus a temporary one-level alias correction during the gate; the
temporary config edit was reverted afterward. No simulator, physical device, real relay process,
or forced live network drop was available, so audible recovery and live OS WebSocket timing were
not manually verified.
