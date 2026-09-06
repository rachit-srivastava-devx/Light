# A6 — relay async I/O timeout — evidence report

Worktree: `company/products/adhd-focus-orb-worktrees/relay-async-io-timeout`
Branch: `fix/relay-async-io-timeout` (base `e942446`)
Crate: `backend/relay-rs`

## Starting state

Two prior workers left real, uncommitted changes in `src/main.rs`, `src/provider.rs`,
`src/session.rs` implementing a `tokio::time::timeout` wrapper around every `spawn_blocking`
provider call (STT push/end-of-turn, TTS synthesis), plus two new acceptance tests:

- `tests::a_hung_stt_call_returns_a_bounded_typed_failure_instead_of_hanging_forever`
- `tests::a_hung_session_does_not_stall_other_sessions_on_the_same_relay`

Both tests hung indefinitely under `cargo test` before this fix (confirmed independently: ran past
30s with zero output past "running 1 test").

## Root cause of the deadlock

The `tokio::time::timeout(provider_timeout, handle).await` wrapper around each `spawn_blocking`
`JoinHandle` is correct on its own terms — it does stop the *async* caller from waiting on a
provider that never returns, and the read loop / session-close path does the right thing. Running
the failing test alone with a shell-level guard proved this:

```
$ timeout 25 cargo test a_hung_stt_call_returns_a_bounded_typed_failure_instead_of_hanging_forever -- --nocapture --test-threads=1
running 1 test
session s1: provider unavailable: provider timed out
session ended: stt_frames=0 tts_chars=0
EXIT: 124                      <-- killed by the shell timeout, AFTER "session ended" printed
```

The test's own async logic (session close, typed `provider_failure` reason, bounded latency) had
already completed and printed its result. The hang was **not** in the code under test — it was in
the test *harness's own teardown*.

`HangingStt::push_audio` (the fake provider added by the acceptance test) calls
`std::thread::park()` with nothing anywhere in the test that ever calls `.unpark()` on that thread
— by design, to model a provider call that a real timeout wrapper genuinely cannot un-stick from the
caller's side (the doc comment on the fixture says as much: "the ONLY thing that can bound this call
from the relay's side is `main.rs`'s own timeout"). `tokio::task::spawn_blocking`'s `JoinHandle`
represents that thread's *eventual* completion; `tokio::time::timeout` racing against it only stops
the *await*, it does not cancel or detach the underlying OS thread — that thread keeps running,
forever, exactly as documented in-repo ("there is no way to kill a synchronous OS thread from
here").

The catch: **`tokio::runtime::Runtime`'s default `Drop` impl blocks the calling thread until every
outstanding `spawn_blocking` task completes** — including ones the async side has already abandoned
via an elapsed `timeout`. `#[tokio::test]` builds exactly such a runtime per test function and drops
it immediately after the test body returns. With `HangingStt`'s thread parked forever, that Drop
never returns, so the *test binary's own teardown* hangs after the test has already, correctly,
passed every assertion — matching exactly the third scenario named in the task brief: "`timeout`
used correctly but ... if the test then tries to join/await something that indirectly waits on that
thread, it hangs" (here the implicit join is tokio's own runtime teardown, not code in `main.rs`).

## Fix

No production code changed (the `PROVIDER_TIMEOUT` / `tokio::time::timeout` wrapper in `main.rs`
was already correct). The fix is confined to the two acceptance tests in
`src/main.rs::tests`, which now run on a hand-built `current_thread` runtime instead of the
`#[tokio::test]`-generated one, and explicitly call `Runtime::shutdown_background()` (detach,
don't join) instead of letting the runtime `Drop` normally:

```rust
fn run_and_detach<F>(fut: F)
where
    F: std::future::Future<Output = ()>,
{
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build test runtime");
    rt.block_on(fut);
    // Not `drop(rt)`: the default Drop impl joins every spawn_blocking task, which would hang
    // forever on the deliberately-immortal thread(s) this test's fake provider leaves behind.
    rt.shutdown_background();
}
```

Both tests changed from `#[tokio::test] async fn …() { … }` to `#[test] fn …() { run_and_detach(async
{ … }); }`. This is safe because `rt.block_on(fut)` already runs the async body — including every
assertion — to completion before `shutdown_background()` is ever called; the parked thread is simply
detached afterward rather than joined, and gets reaped by the OS when the whole test process exits.

No other test in the file needed this treatment — they don't leave a `spawn_blocking` task
permanently parked, so the ordinary `#[tokio::test]` teardown completes normally for them.

## Verification

### The two previously-hanging tests, individually, with an explicit shell timeout guard

```
$ timeout 20 cargo test a_hung_stt_call_returns_a_bounded_typed_failure_instead_of_hanging_forever -- --nocapture
running 1 test
session s1: provider unavailable: provider timed out
session ended: stt_frames=0 tts_chars=0
test tests::a_hung_stt_call_returns_a_bounded_typed_failure_instead_of_hanging_forever ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 56 filtered out; finished in 0.09s
EXIT: 0
```

```
$ timeout 20 cargo test a_hung_session_does_not_stall_other_sessions_on_the_same_relay -- --nocapture
running 1 test
session s1: provider unavailable: provider timed out
session ended: stt_frames=0 tts_chars=0
test tests::a_hung_session_does_not_stall_other_sessions_on_the_same_relay ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.05s
EXIT: 0
```

Both complete on their own (exit 0), well under the shell timeout guard — not killed externally.

### Full `cargo test` suite, twice, with a shell timeout guard

```
$ timeout 120 cargo test
running 57 tests
... (all 57 listed ok, including both hang-scenario tests) ...
test result: ok. 57 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.11s
EXIT: 0
```

Re-ran a second time: identical result, 5.11s, exit 0 (see `cargo_test_full.log` in this directory).
No hangs anywhere in the suite.

### `npm run verify` from the worktree root

```
$ npm run verify
> lint            -> boundary-lint: clean
> typecheck        -> tsc --noEmit -p apps/mobile && ... -> EXIT 2, 20x error TS... in
                       apps/mobile/src/runtime/T0FocusSession.ts and
                       registry/features/realtime-voice/src/... (Cannot find module
                       '@pe/voice-realtime', tenant_id/session_id property errors)
verify EXIT: 2
```

Full output: `verify.log` in this directory.

**Isolated the failure as pre-existing and unrelated to this fix**, per the fleet convention
(disposable detached worktree, not `git stash` — stash is shared across worktrees of this repo and
has caused real collisions this session, see `FLEET-LEARNINGS.md`):

```
$ git worktree add --detach <scratch> e942446
$ ln -s <this-worktree>/node_modules <scratch>/node_modules   # avoid a full reinstall
$ cd <scratch> && npm run typecheck
EXIT 2, 20x error TS...
```

Diffed the two error lists (stripped of file-line-column prefixes) — **identical**, confirming the
`typecheck` failure is a pre-existing gap (the `@pe/voice-realtime` / `@pe/llm-gateway`
module-resolution issue other workers on this branch set have already documented in
`FLEET-LEARNINGS.md`) and has nothing to do with the Rust timeout fix. `npm run test:rs` (the step
`verify` would reach after `typecheck` if it didn't halt there) is exactly the `cargo test` run
above — 57/57 passed.

Cleaned up the scratch worktree afterward (`git worktree remove --force <scratch>`).

## Files changed

- `backend/relay-rs/src/main.rs` — no production-path changes beyond what the prior workers already
  landed (`PROVIDER_TIMEOUT`, the `tokio::time::timeout` wrapper in `stt_loop` and `spawn_speech`,
  `Connection::provider_timeout` threading). Test-only change: added `run_and_detach` helper;
  converted the two hang-scenario tests from `#[tokio::test]` to `#[test]` + `run_and_detach`.
- `backend/relay-rs/src/provider.rs` — unchanged from the prior workers' diff (`ProviderError::TimedOut`).
- `backend/relay-rs/src/session.rs` — unchanged from the prior workers' diff (`SttFailure::from` mapping).

Full diff: `final.diff` in this directory.

## Reproduce from a fresh clone

```bash
cd backend/relay-rs
timeout 20 cargo test a_hung_stt_call_returns_a_bounded_typed_failure_instead_of_hanging_forever -- --nocapture
timeout 20 cargo test a_hung_session_does_not_stall_other_sessions_on_the_same_relay -- --nocapture
timeout 120 cargo test
```

All three should exit 0 well inside their guard timeouts.

For `npm run verify`, run it from the worktree root; expect it to halt at `typecheck` with the
pre-existing `@pe/voice-realtime` module-resolution errors (unrelated to this change) — confirm by
diffing against a disposable detached worktree of `e942446` as done above.

## What is not covered

- The pre-existing `npm run verify` `typecheck` failure (module resolution for
  `@pe/voice-realtime`/`@pe/llm-gateway`) is out of scope for A6 and was not touched — it blocks
  `verify` from ever reaching `test`, `test:py`, `test:rs` as a single combined command, so those
  were run directly instead (`cargo test`, matching what `npm run test:rs` invokes).
- Did not attempt to make `Runtime::drop` itself safe against a permanently-parked `spawn_blocking`
  thread in general — that is inherent to any Tokio runtime and not something a caller can change;
  the fix here is scoped to the two tests that deliberately create such a thread.
