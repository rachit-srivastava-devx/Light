# A7 — tts-true-streaming

Worktree: `company/products/adhd-focus-orb-worktrees/tts-true-streaming` (branch `fix/tts-true-streaming`,
base `e942446`).

## Defect

TTS was fully buffered before send despite a "chunked" audio-array protocol already existing.
Found via `grep -r chunked backend/` and `grep -rli tts backend/`, which led to
`backend/relay-rs/src/provider.rs`'s own module doc, which said so explicitly:

> "Note what this boundary is NOT: the TTS response is one whole JSON body, so there is no
> streaming here and no first-chunk-early."

Three converging facts:

1. `TtsProvider::synthesize()` returned `Result<Vec<Vec<u8>>, ProviderError>` -- a single blocking
   call that could only ever hand back every chunk at once, however capable the underlying
   provider was. No implementation of this trait could stream even in principle.
2. `HttpContractProvider::synthesize` (the real, non-fake production path) read the *entire* HTTP
   response (`Content-Length` framed, one JSON body `{ "audio_chunks": [[...], ...] }`) before
   parsing anything.
3. `main.rs`'s `spawn_speech` called `providers.synthesize(...)` and only started forwarding audio
   to the client's WebSocket in a `for chunk in chunks` loop *after* that whole call returned.

So even though the wire format already looked "chunked" (an array of arrays), nothing downstream
of provider dispatch could act on chunk 1 until chunk N existed.

## Fix

- `backend/relay-rs/src/provider.rs`: changed the `TtsProvider` trait to
  `fn synthesize(&self, text, voice_id, emotion, cancel, on_chunk: &mut dyn FnMut(Vec<u8>) -> Result<(), ProviderError>) -> Result<(), ProviderError>`
  -- a push callback invoked once per chunk as it becomes available, not a return value assembled
  at the end. `FakeProvider` calls it twice immediately (unchanged shape, new signature).
  `HttpContractProvider::synthesize` was rewritten (`synthesize_streaming`) to speak real HTTP/1.1
  chunked transfer-encoding: it parses the response as bytes arrive off the socket -- chunk-size
  line, then that many bytes, repeat -- and calls `on_chunk` per HTTP wire-chunk, never waiting for
  Content-Length or a terminating body close.
- `backend/relay-rs/src/main.rs`: `spawn_speech`'s inner `on_chunk` closure now forwards each
  chunk to the outbound WebSocket queue (via a polled `try_send` retry, not a single
  `blocking_send`, so a barge-in can still interrupt while backpressured) the moment the provider
  produces it, tracking `first_chunk_at`/`audio_chunks`/`audio_bytes` telemetry per chunk instead
  of after the fact. `SlowTts`/`SlowStt` test fixtures were mechanically adapted to the new
  signature (unchanged behaviour). Added `StreamingTts`, a test provider whose synthesis genuinely
  trickles out one chunk every `chunk_delay` -- the shape a real streaming TTS provider has -- used
  only by the new acceptance test.
- `backend/relay-rs/src/cancel.rs`: `CancelFlag::cancelled()` (the async wait future) is no longer
  called from `main.rs` (the send is now a synchronous poll loop on a blocking thread, matching
  `HttpContractProvider`'s existing style) -- kept, documented, `#[allow(dead_code)]`, still
  exercised by its own tests.
- `backend/voice-provider-sidecar/src/index.ts`: `handleTtsSynthesize` now calls a new
  `writeTtsChunks()` which streams the response as `Transfer-Encoding: chunked`, one HTTP chunk per
  audio chunk (`{"chunk":[...]}\n`), instead of `sendJson()`-ing one whole body. This is the
  production wire format `HttpContractProvider::synthesize_streaming` now requires; without this
  change the real (non-fake) HTTP provider path would be **broken**, not just slow, since the new
  Rust client rejects a response that isn't `Transfer-Encoding: chunked`.
- `backend/voice-provider-sidecar/src/contract.ts`: doc comment + `TtsSynthesizeResponse` ->
  `TtsChunkFrame` updated to match the new wire shape (that interface had no other callers).
- `backend/voice-provider-sidecar/__tests__/http-contract.test.ts`: the TTS synthesize test now
  asserts `transfer-encoding: chunked` and reassembles the de-chunked body as newline-delimited
  JSON instead of `res.json()`-ing a single object (the old assertion, unchanged, would now fail --
  there is no single JSON object in the response any more).

## Honest scope note

The underlying paid TTS backends in this repo (`tts/fish.ts`, `tts/cartesia.ts`, `tts/sarvam.ts`)
each make ONE non-streaming HTTP request and `await` the full response before returning -- Cartesia
in particular calls `/tts/bytes`, not a streaming endpoint. This fix removes every structural
buffering point *downstream* of that call (provider trait, relay/sidecar wire contract, WebSocket
send path), so a genuinely streaming provider integration would now deliver its first chunk to the
client immediately. It does **not** rewire Fish/Cartesia/Sarvam to their streaming APIs -- that is a
separate, larger change (different SDK surface per provider) and out of this task's scope. This gap
was already true before this fix and is unchanged by it; flagging it rather than passing it off as
solved.

## RED, confirmed

The two new tests need the new trait signature to compile, so RED was demonstrated the same way
the audio-bed-fallback worker did this session: temporarily reintroduce the exact defect (buffer
every chunk in `spawn_speech`'s `on_chunk` closure into a `Vec`, flush it only after
`providers.synthesize()` fully returns) on top of the new interface, run the new tests, then revert
to the real fix. Command and output (`cargo test -- first_audio_chunk_is_sent_before
tts_first_chunk_is_delivered`, from `backend/relay-rs`):

```
running 2 tests
test provider::tests::tts_first_chunk_is_delivered_before_the_last_chunk_is_even_written ... ok
test tests::first_audio_chunk_is_sent_before_the_full_utterance_finishes_synthesizing ... FAILED

failures:

---- tests::first_audio_chunk_is_sent_before_the_full_utterance_finishes_synthesizing stdout ----

thread 'tests::first_audio_chunk_is_sent_before_the_full_utterance_finishes_synthesizing' panicked at src/main.rs:1751:9:
first chunk took 1.01358275s to arrive; a provider streaming one chunk every 200ms should deliver the first one in around 200ms, not after all 5 were synthesized (~1000ms) -- the send path is still fully buffering

test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 55 filtered out; finished in 1.34s
```

(The HTTP-level test, `tts_first_chunk_is_delivered_before_the_last_chunk_is_even_written`, still
passed under this shim -- it exercises `HttpContractProvider` directly against a raw socket server,
a different layer than `spawn_speech`'s send loop, so it is unaffected by that specific temporary
regression. It is its own independent proof that the HTTP client correctly streams.)

The shim was then reverted (restored from a pre-fix backup copy) and the suite re-run for GREEN.

## GREEN, full commands and real output

### `cargo test` (from `backend/relay-rs`) -- 57 passed, was 55 at base `e942446`

```
$ cd backend/relay-rs && cargo test
running 57 tests
test latency_budget::tests::boundary_values_are_inclusive_of_the_stricter_bucket ... ok
test cancel::tests::starts_uncancelled_and_latches_once_cancelled ... ok
test cancel::tests::a_clone_observes_the_original_cancel ... ok
test cancel::tests::blocking_code_sees_it_through_the_provider_trait ... ok
test latency_budget::tests::conversational_turn_breached_over_p99 ... ok
test latency_budget::tests::conversational_turn_within_p50 ... ok
test cancel::tests::cancelled_resolves_immediately_when_already_cancelled ... ok
test latency_budget::tests::deterministic_turn_breached ... ok
test latency_budget::tests::every_hot_path_constant_is_consumed_by_the_budget_lookup ... ok
test latency_budget::tests::deterministic_turn_within_p50 ... ok
test latency_budget::tests::deterministic_turn_within_p99_but_over_p50 ... ok
test latency_budget::tests::first_audio_and_think_time_fail_above_250_ms ... ok
test protocol::tests::close_reason_distinguishes_pause_from_failure ... ok
test protocol::tests::client_frames_round_trip_through_json ... ok
test provider::tests::a_cancelled_stt_push_is_not_reported_as_an_outage ... ok
test provider::tests::a_third_party_adapter_satisfies_the_provider_boundary ... ok
test protocol::tests::frames_are_tagged_by_type_so_the_client_can_switch_on_it ... ok
test provider::tests::a_down_provider_reports_an_outage_rather_than_silence ... ok
test protocol::tests::unknown_frame_type_is_rejected_rather_than_silently_ignored ... ok
test provider::tests::default_provider_is_explicitly_t0_fake ... ok
test provider::tests::external_selection_requires_injected_adapter ... ok
test provider::tests::http_selection_requires_a_base_url ... ok
test provider::tests::synthesis_already_cancelled_never_reaches_the_provider ... ok
test session::tests::a_barge_in_after_dispatch_still_bills_what_the_provider_was_asked_for ... ok
test session::tests::a_cancelled_stt_call_is_not_an_outage ... ok
test session::tests::a_barge_in_that_beats_dispatch_bills_nothing ... ok
test session::tests::a_failed_synthesis_for_a_superseded_generation_does_not_close_the_session ... ok
test session::tests::a_failed_synthesis_for_the_live_generation_closes_the_session ... ok
test session::tests::a_second_speak_supersedes_the_first_generation ... ok
test session::tests::audio_before_start_listening_is_ignored ... ok
test session::tests::accepts_the_next_user_turn_after_speech_completes ... ok
test session::tests::audio_frames_dispatch_to_stt_and_surface_partials ... ok
test session::tests::barge_in_yields_the_floor_and_retires_the_live_generation ... ok
test session::tests::frames_after_a_pause_are_ignored_not_processed ... ok
test session::tests::end_of_turn_dispatches_the_finalise_off_the_read_loop ... ok
test provider::tests::http_contract_provider_uses_stt_and_tts_contracts ... ok
test session::tests::mic_audio_is_still_accepted_while_the_orb_is_speaking ... ok
test session::tests::mismatched_tenant_frame_is_ignored ... ok
test session::tests::provider_error_maps_cancellation_apart_from_outage ... ok
test session::tests::nothing_is_captured_or_billed_after_a_pause ... ok
test session::tests::pause_closes_the_session_with_the_user_pause_reason ... ok
test session::tests::provider_failure_closes_with_provider_failure_not_user_pause ... ok
test session::tests::speak_requests_synthesis_and_bills_characters_only_once_delivered ... ok
test cancel::tests::cancelled_does_not_lose_a_cancel_racing_the_registration ... ok
test cancel::tests::cancelled_wakes_a_waiter_registered_before_the_cancel ... ok
test provider::tests::an_in_flight_synthesis_is_abandoned_when_the_flag_flips ... ok
test tests::pause_during_synthesis_closes_without_playing_the_utterance ... ok
test tests::the_writer_drops_audio_whose_generation_is_no_longer_live ... ok
test tests::the_writer_drops_only_the_retired_generation_not_the_live_one ... ok
test tests::the_writer_sends_audio_whose_generation_is_still_live ... ok
test tests::a_barge_in_is_not_starved_by_a_backlog_of_mic_frames ... ok
test tests::barge_in_during_synthesis_delivers_no_audio_at_all ... ok
test tests::no_audio_ever_arrives_after_the_yield_is_acknowledged ... ok
test provider::tests::tts_first_chunk_is_delivered_before_the_last_chunk_is_even_written ... ok
test tests::an_uninterrupted_utterance_is_delivered_in_full ... ok
test tests::a_second_speak_never_interleaves_the_first_utterances_audio ... ok
test tests::first_audio_chunk_is_sent_before_the_full_utterance_finishes_synthesizing ... ok

test result: ok. 57 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.35s
```

Also confirmed via the npm wrapper: `npm run test:rs` -> `test result: ok. 57 passed; 0 failed`.

### `npx vitest run backend/voice-provider-sidecar` -- 57 passed

```
 Test Files  7 passed (7)
      Tests  57 passed (57)
```
(includes the updated `POST /v1/tts/synthesize streams one HTTP chunk per audio chunk, not one
whole JSON body` test, and the other 6 unrelated sidecar suites, all green.)

### `npm run test` (whole-repo vitest) -- 616 tests passed; 6 suites fail to load, all pre-existing

```
 Test Files  6 failed | 48 passed (54)
      Tests  616 passed (616)
```
The 6 failed suites are `apps/mobile/src/AppSurface.test.tsx`, `AppVoiceWiring.test.tsx`,
`runtime/T0FocusSession.test.ts`, `runtime/VoiceLoopController.test.ts`,
`backend/gateway-sidecar/__tests__/adapterTelemetry.test.ts`, `complete.test.ts` -- all fail to
*import*, not to run, on `Cannot find module '@pe/voice-realtime/adapters/memory'` /
`'@pe/llm-gateway/adapters/memory'`. None of these files, or the registry packages they import,
were touched by this change (`backend/relay-rs`, `backend/voice-provider-sidecar` only). This is
the same `@pe/voice-realtime`/`@pe/llm-gateway` gap multiple prior workers in this session recorded
as pre-existing at `e942446` (see `FLEET-LEARNINGS.md`, "no-hardcoded-transcript" and
"no-fake-tts-default" entries). Zero test *cases* failed; only these 6 suites failed to load.

### `npm run test:py` -- 393 passed (untouched by this change; relay-py wasn't modified)

```
393 passed, 3 warnings in 6.19s
```

### `npm run verify` -- exits non-zero; isolated to the pre-existing typecheck gap

`npm run verify` chains `lint && typecheck && test && test:py && test:rs` with `&&`, so it stops at
the first red step. `typecheck` (`tsc -p apps/mobile`) fails with the exact same 20-line error set
listed above (`@pe/voice-realtime`/`VoiceFeatureInput` missing `tenant_id`/`session_id`), which
**never reaches `backend/relay-rs` or `backend/voice-provider-sidecar` typechecking** (that's a
separate `tsc -p` invocation later in the same npm script, run and confirmed clean separately --
`npx tsc --noEmit -p backend/voice-provider-sidecar` exits 0).

Isolation, done the way this session's other workers did it: `git stash push` on exactly the 6
files this task touched, re-ran `npx tsc --noEmit -p apps/mobile` against the unmodified worktree
(effectively base `e942446`, since nothing else in this worktree was touched), and the error list
was **byte-for-byte identical** to the one with the fix applied. `git stash pop` restored the fix.
`lint` (`boundary-lint`) passed clean both times.

Net: everything this task's diff touches (`backend/relay-rs`, `backend/voice-provider-sidecar`) is
green -- `cargo test` 57/57, sidecar vitest 57/57, `tsc -p backend/voice-provider-sidecar` clean,
`boundary-lint` clean. `npm run verify`'s red exit code is entirely the pre-existing, previously
documented `apps/mobile`/`gateway-sidecar` module-resolution gap, unrelated to this diff.

## Files changed

- `backend/relay-rs/src/provider.rs`
- `backend/relay-rs/src/main.rs`
- `backend/relay-rs/src/cancel.rs`
- `backend/voice-provider-sidecar/src/index.ts`
- `backend/voice-provider-sidecar/src/contract.ts`
- `backend/voice-provider-sidecar/__tests__/http-contract.test.ts`

## Reproduce from a fresh clone

```
cd company/products/adhd-focus-orb-worktrees/tts-true-streaming
ln -s ../../adhd-focus-orb/node_modules node_modules                       # gitignored, needed for npm/tsc/vitest
ln -s ../../../../adhd-focus-orb/backend/relay-py/.venv backend/relay-py/.venv  # gitignored, needed for pytest

cd backend/relay-rs && cargo test                    # expect: 57 passed
cd ../..
npx vitest run backend/voice-provider-sidecar         # expect: 57 passed
npm run test                                          # expect: 616 tests passed, 6 pre-existing suite-load failures
npm run test:py                                        # expect: 393 passed
npx tsc --noEmit -p backend/voice-provider-sidecar     # expect: clean, exit 0
node tooling/boundary-lint.mjs                         # expect: clean
```

## Anything unverified

- No live/paid TTS provider (Fish/Cartesia/Sarvam) was exercised -- all evidence above is against
  the fake backend and in-process test doubles. The wire-level streaming fix (relay <-> sidecar) is
  proven with real HTTP chunked transfer-encoding over a real TCP socket in
  `provider::tests::tts_first_chunk_is_delivered_before_the_last_chunk_is_even_written`, but no
  audio was played on a device -- this is the same class of gap flagged in every other worker's
  entry this session (audible/manual verification not exercised).
- The paid-provider integrations (`fish.ts`/`cartesia.ts`/`sarvam.ts`) still make one non-streaming
  HTTP call each; see "Honest scope note" above -- this fix removes the buffering downstream of that
  call but does not make those specific SDK calls themselves stream.
- `npm run verify`'s own exit code was not driven to 0 in this worktree, because the pre-existing
  `apps/mobile`/`gateway-sidecar` typecheck gap sits upstream of this task's files in that chained
  script; isolation evidence above shows it is unrelated and pre-existing, not caused by this diff.
