#!/usr/bin/env node
/**
 * Measure barge-in for real. This has never been measured in this product before this file:
 *
 *   - `scripts/voice-ux-gate.mjs` G5 finds ~1 barge-in dispatch sample in the whole dev-logs/
 *     corpus (tens of thousands of rows) and fails loudly with "measured nothing" rather than
 *     reporting a meaningless 100% off one row.
 *   - `audio-gate/README.md` states outright: "actual rendered TTS yield within 100 ms is not yet
 *     measured."
 *   - Every barge-in screenshot in `e2e-human-simulator/runs/**\/trace.jsonl` is tagged
 *     `"ui_confirmed": false` — a human still has to look at it.
 *
 * WHAT THIS FILE ACTUALLY MEASURES, PRECISELY, AND WHY THAT PRECISION MATTERS
 * -----------------------------------------------------------------------------
 * This drives the real relay-rs over its real WebSocket, with real Fish TTS audio (no fakes, no
 * stubs), and times, on ITS OWN clock, the interval from sending an interruption control frame to
 * the arrival of the LAST binary audio frame that frame's in-flight `speak` produces.
 *
 * That number is **relay-side frame-stop latency — a transport-level measurement**. It is NOT
 * audio-yield-at-the-speaker. Nothing in this file touches a speaker, a microphone, or the real
 * mobile client (`apps/mobile/src/orb/`, `apps/mobile/src/voice/BargeIn.ts`,
 * `RelayAudioPlayer.ts`) — this is a headless WebSocket client, not a phone. See the "WHAT THIS
 * MEASUREMENT CANNOT SEE" section printed at the end of every run (and in
 * e2e-human-simulator/README.md) for the three-layer breakdown of what a barge-in actually is
 * (relay stops sending -> client stops playing -> speaker goes quiet) and which one this number
 * covers (only the first).
 *
 * WHY THE RELAY'S OWN ARCHITECTURE MAKES THIS MEASUREMENT INTERESTING
 * -----------------------------------------------------------------------------
 * Read end to end before changing this file: `backend/relay-rs/src/main.rs` `serve_connection`
 * runs ONE sequential loop — `while let Some(message) = source.next().await`. `Speak` is handled
 * by calling `tts.synthesize()` **synchronously, with no `.await`** (`session.rs`'s own doc
 * comment: "Synthesis is synchronous inside this relay handler"), which for the configured
 * `ORB_RELAY_PROVIDER=http` backend is an HTTP round trip to `voice-provider-sidecar` that returns
 * the ENTIRE audio payload as one JSON body (`HttpContractProvider::synthesize`,
 * `backend/relay-rs/src/provider.rs:236-271` — there is no streaming at this boundary). Once that
 * returns, `main.rs`'s `Action::SendWithAudio` branch (lines 358-373) sends every chunk in a tight
 * `for chunk in audio_chunks { sink.send(...).await?; }` loop — and the loop never calls
 * `source.next()` again until every chunk (and the trailing `speech_complete` frame) has been
 * flushed. **A client frame sent while that loop is running is provably unreadable by the relay
 * until the loop finishes**, regardless of which frame type it is or how "urgent" its doc comment
 * says it is. That is the hypothesis this script tests empirically rather than asserts from
 * reading the code — see the per-sample `frames_after` column and the control trial below.
 *
 * 2026-08-28 UPDATE — THE ARCHITECTURE ABOVE WAS FIXED, AND WHAT THIS FILE GRADES CHANGED
 * -----------------------------------------------------------------------------
 * The paragraph above described `backend/relay-rs` as it stood when this file was written, and the
 * first real run of this script confirmed it: p50 1897.7ms / p95 2291.8ms against a 100ms budget,
 * with a control trial receiving statistically the SAME bytes as an interrupted one (-0.6%) —
 * barge_in truncated nothing. `serve_connection` has since been restructured (read/write split,
 * synthesis on its own cancellable task, generation-tagged audio dropped before the socket).
 *
 * That fix breaks the ORIGINAL metric, and the fix to the metric is documented here rather than
 * quietly applied. The original headline number was `frameStopLatencyMs` = "interrupt sent -> last
 * binary frame of that turn". When the relay cancels correctly during synthesis, ZERO audio frames
 * are ever sent, so that quantity has no value at all — and the original code path scored such a
 * trial as `ok: false` ("no binary audio frame arrived"), i.e. it scored the strongest possible
 * pass as an unusable sample. The graded metric is therefore now:
 *
 *     yieldLatencyMs = (interrupt sent) -> the LATER of
 *                        (a) the last audio byte the relay sends for the interrupted turn, and
 *                        (b) the relay's own acknowledgement that it yielded
 *                            (barge_in -> `speech_complete`; pause -> `closing`).
 *
 * The test that this is a fix and not a goalpost move: **it produces the same FAIL on the old
 * code.** Under the old sequential loop the first `speech_complete` after the interrupt is the
 * interrupted turn's OWN completion frame, emitted only after the whole burst has drained — so (b)
 * lands at ~1.9s just as (a) did. A/B evidence for both binaries is in the run log accompanying
 * this change. `frameStopLatencyMs` is still computed and still printed next to it, so no number
 * that was reported before has been removed.
 *
 * FRAME TYPES UNDER TEST — grounded in `backend/relay-rs/src/protocol.rs`, not guessed
 * -----------------------------------------------------------------------------
 * `barge_in` and `pause` are the only two frames protocol.rs documents as interruption-shaped
 * (see FRAME_TYPES below for the exact doc comments, quoted with line numbers). `end_of_turn` and
 * `start_listening` are STT turn-lifecycle frames, not documented as TTS interruptions — they are
 * tested anyway because the brief for this file named them explicitly, and because "does sending
 * this during Speaking do anything at all" is itself a fact worth having on the record rather than
 * assuming.
 *
 * REUSE, NOT REINVENTION
 * -----------------------------------------------------------------------------
 * WebSocket connect/send/receive plumbing is copied in shape from `synthetic_mic.mjs` and
 * `conversation_drive.mjs` (global `WebSocket`, `binaryType = 'arraybuffer'`, JSON control frames,
 * poll-with-timeout waits) — both already work end to end against this exact relay. The dev-logs
 * NDJSON reader is imported directly from `scripts/voice-ux-gate.mjs`'s exported `readRows` (same
 * function G5 itself uses) rather than reimplemented, so the optional cross-check against the
 * relay's own `frame.processed` events is reading the same corpus the same way the gate does.
 *
 * FAIL-CLOSED
 * -----------------------------------------------------------------------------
 * Zero audio frames ever, for any frame type -> exit 6. Fewer than MIN_USABLE_SAMPLES (10) usable
 * trials for any frame type -> exit 6. barge_in's own p95 over its budget -> exit 6. A run that
 * measured nothing, or measured too little to trust, must never exit 0 — see printReport().
 *
 * Usage
 *   node e2e-human-simulator/barge_in_drive.mjs
 *   BARGE_IN_SAMPLES=20 node e2e-human-simulator/barge_in_drive.mjs
 *   BARGE_IN_FRAME_TYPES=barge_in node e2e-human-simulator/barge_in_drive.mjs   # cheap, one path
 *   node e2e-human-simulator/barge_in_drive.mjs --json                          # machine-readable
 *
 * Money: this spends real Fish TTS credit (ORB_TTS_PROVIDER=fish, s2-pro). Cost is computed from
 * characters actually synthesized at this product's own configured rate
 * (backend/relay-py/src/orb_relay/app.py:229, ORB_RATE_PAISE_PER_1K_TTS_CHARS, default 143
 * paise/1000 chars) and printed at the end of every run — see printCostReport().
 */

import { performance } from 'node:perf_hooks';
import { pathToFileURL } from 'node:url';

// --- config, all overridable, matching this directory's existing env-var convention exactly ------

const RELAY_WS = process.env.ORB_RELAY_WS_URL ?? 'ws://127.0.0.1:8091';
const TENANT = 't0';
const RUN_ID = Date.now();
const DEV_LOG_DIR = process.env.ORB_DEV_LOG_DIR ?? 'dev-logs';

const SAMPLES_PER_TYPE = envInt('BARGE_IN_SAMPLES', 10);
const CONTROL_SAMPLES = envInt('BARGE_IN_CONTROL_SAMPLES', 5);
const MIN_USABLE_SAMPLES = 10; // fail-closed floor — the task's own "at least 10" instruction,
                                // applied as a data-integrity floor regardless of SAMPLES_PER_TYPE.
const REQUESTED_FRAME_TYPES = (process.env.BARGE_IN_FRAME_TYPES ?? 'barge_in,pause,end_of_turn,start_listening')
  .split(',').map((s) => s.trim()).filter(Boolean);

const QUIET_WINDOW_MS = 1200;      // no new frame for this long after the interrupt => call it over
const TRIAL_HARD_TIMEOUT_MS = 20000; // absolute cap so a hung provider can't hang the whole run
const FIRST_AUDIO_TIMEOUT_MS = 15000; // real Fish TTS network call; generous but bounded
const CONNECT_TIMEOUT_MS = 8000;
const INTER_TRIAL_PAUSE_MS = 350;   // be polite to the real Fish TTS API between calls
const IMMEDIATE_INTERRUPT_DELAY_MS = 50; // 'immediate' timing: fire this long after `speak`, while
                                          // the relay is provably still blocked in synthesis (see
                                          // the block comment above runTrial()).
const SECONDARY_FIRST_AUDIO_SAMPLES = envInt('BARGE_IN_FIRST_AUDIO_SAMPLES', 5); // barge_in only

// A single fixed utterance for every trial (calibration, control, and all interrupt trials), so
// every sample is measuring the same synthesis workload. ~178 chars / ~31 words: long enough that
// a real TTS reply is several seconds of audio (100+ binary chunks at the relay's own 3200-byte
// chunking, backend/voice-provider-sidecar/src/index.ts:106-107) — short enough to stay cheap (see
// printCostReport). Product-voice-appropriate text, matching the tone of the other e2e scripts.
const UTTERANCE = "Okay, I hear you — the kitchen's a mess and the laundry's piling up, but "
  + "none of that has to happen all at once. Let's just pick one small thing and start there "
  + 'together.';

// This product's own barge-in yield budget. The operative constant the relay actually compiles
// against is backend/relay-rs/src/latency_budget.rs:18
// (`pub const BARGE_IN_YIELD_BUDGET_MS: u64 = 100;`), whose own doc comment says it is "quoted
// verbatim from docs/BUILD-DIGEST.md §3" (docs/BUILD-DIGEST.md:191-192: "barge-in yield ≤100ms").
// The ORIGINATING spec statement is the blueprint:
// blueprints/ADHD-Focus-Orb-L8-Deep-Dive/03-VOICE-LATENCY-PIPELINE.md:167 ("On user voice onset,
// **yield ≤ 100 ms**"). Restated as a numeric acceptance anchor in AGENTS.md:88 ("barge-in yield
// ≤100ms") and this product's own CLAUDE.md:43 ("barge-in ≤100ms"). All four numbers agree: 100ms.
const BARGE_IN_YIELD_BUDGET_MS = 100;
const ORB_RATE_PAISE_PER_1K_TTS_CHARS = 143; // backend/relay-py/src/orb_relay/app.py:229 default
const INR_PER_USD = 95; // this repo's own stated July-2026 convention

// Frame types under test, grounded in backend/relay-rs/src/protocol.rs's ClientFrame enum (the
// exact doc comments are quoted below, with line numbers, per the brief: "ground every frame name
// in that file; do not guess one").
const FRAME_TYPES = {
  barge_in: {
    type: 'barge_in',
    docComment: '"The user started speaking over the orb. Must yield within '
      + 'BARGE_IN_YIELD_BUDGET_MS." (protocol.rs:31-35) — the only frame with an explicit numeric '
      + 'budget attached to it in the protocol source.',
    interruptionDocumented: true,
    // What the relay sends to say it has yielded. session.rs's BargeIn arm returns
    // `Action::YieldSpeech([SpeechComplete])`, so this is the observable that exists in BOTH the
    // old and the fixed relay — which is what makes it usable as a before/after metric.
    yieldAckTypes: ['speech_complete'],
    payload: (session) => ({ type: 'barge_in', tenant_id: TENANT, session_id: session }),
  },
  pause: {
    type: 'pause',
    docComment: '"User-intent pause (§5): background, screen lock, BT disconnect. Total and '
      + 'immediate — the socket closes, nothing is buffered, nothing further is billed." '
      + '(protocol.rs:36-41). Structurally different from barge_in: this closes the whole '
      + 'connection rather than returning to Listening.',
    interruptionDocumented: true,
    // Pause closes the session, so its acknowledgement is the `closing` frame, not speech_complete.
    yieldAckTypes: ['closing'],
    payload: (session) => ({ type: 'pause', tenant_id: TENANT, session_id: session }),
  },
  end_of_turn: {
    type: 'end_of_turn',
    docComment: '"The user stopped speaking (semantic endpointer fired on-device)." '
      + '(protocol.rs:18-22) — an STT turn-boundary signal. NOT documented as a TTS '
      + 'interruption; tested because the brief named it explicitly. This session never streamed '
      + 'any mic audio, so the STT provider may report failure for a turn it never opened — '
      + 'that outcome is itself recorded, not treated as a script bug.',
    interruptionDocumented: false,
    // Not an interruption, so there is nothing to acknowledge: these trials are still graded the
    // original way (last binary frame), and a trial with no audio at all is still a failure.
    yieldAckTypes: null,
    payload: (session) => ({ type: 'end_of_turn', tenant_id: TENANT, session_id: session }),
  },
  start_listening: {
    type: 'start_listening',
    docComment: '"Opens the STT stream for a session. Must be the first frame." (protocol.rs:13-17)'
      + ' — a session-open signal. NOT documented as a TTS interruption; tested because the '
      + 'brief named it explicitly. Re-sending it with the SAME identity produces `Action::Send('
      + 'vec![])` — literally no acknowledgement frame at all (session.rs:126-144) — so '
      + 'this frame type’s processing can only be inferred from its (in)effect on audio, never '
      + 'confirmed by an ack.',
    interruptionDocumented: false,
    yieldAckTypes: null,
    payload: (session) => ({ type: 'start_listening', tenant_id: TENANT, session_id: session }),
  },
};

// --- small utilities --------------------------------------------------------------------------

function envInt(name, fallback) {
  const raw = process.env[name];
  if (raw === undefined || raw === '') return fallback;
  const n = Number.parseInt(raw, 10);
  return Number.isFinite(n) && n > 0 ? n : fallback;
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const nowMs = () => performance.now();

/** Nearest-rank percentile, identical method to scripts/voice-ux-gate.mjs's own G5 computation
 * (`latencies[Math.min(latencies.length - 1, Math.floor(p * latencies.length))]`) — same corpus
 * conventions, same math, so the two numbers are comparable in shape even though they measure
 * different layers. */
function percentile(sortedAscending, p) {
  if (sortedAscending.length === 0) return null;
  const idx = Math.min(sortedAscending.length - 1, Math.floor(p * sortedAscending.length));
  return sortedAscending[idx];
}

const fmt = (ms) => (ms === null || ms === undefined || Number.isNaN(ms) ? 'n/a' : `${ms.toFixed(1)}ms`);
const shortSid = (sid) => sid.replace('bargein-drive-', '').replace(`-${RUN_ID}`, '');

/** Timestamp of the first server text frame of one of `types` that arrived strictly after `since`.
 * This is how the relay says "I have yielded the floor": `speech_complete` for barge_in, `closing`
 * for pause. Returns null if it never said so. Parsing is by JSON.parse on the frame, not substring
 * matching, so a transcript that merely CONTAINS the word cannot be mistaken for an ack. */
/** Has this turn's delivery actually started or finished? Only then does a quiet gap mean "the
 * burst ended" rather than "synthesis has not answered yet". A binary frame means audio is
 * flowing; `speech_complete`/`closing` means the turn is over. `speech_starting` means the exact
 * opposite — audio is coming, keep waiting — so it deliberately does NOT open the quiet window. */
function deliveryUnderway(events) {
  for (const e of events) {
    if (e.kind === 'binary') return true;
    if (e.kind !== 'text') continue;
    try {
      const t = JSON.parse(e.text).type;
      if (t === 'speech_complete' || t === 'closing') return true;
    } catch { /* unparseable frames say nothing about delivery */ }
  }
  return false;
}

function countTextTypes(events) {
  const counts = {};
  for (const e of events) {
    if (e.kind !== 'text') continue;
    try {
      const t = JSON.parse(e.text).type;
      counts[t] = (counts[t] ?? 0) + 1;
    } catch { counts.unparseable = (counts.unparseable ?? 0) + 1; }
  }
  return counts;
}

function firstAckAfter(events, types, since) {
  for (const e of events) {
    if (e.kind !== 'text' || e.ts <= since) continue;
    let parsed;
    try { parsed = JSON.parse(e.text); } catch { continue; }
    if (types.includes(parsed?.type)) return e.ts;
  }
  return null;
}

// --- the relay transport: reused connect/send/receive shape from synthetic_mic.mjs --------------

/** Opens a fresh WebSocket, wires collectors BEFORE the open promise resolves (same ordering as
 * synthetic_mic.mjs/conversation_drive.mjs), and returns live-updating state the caller polls. A
 * fresh socket per trial, never reused — same reasoning conversation_drive.mjs documents: it
 * mirrors an app that opens a session per utterance and avoids any state bleeding across trials. */
function openSession(sessionId) {
  const ws = new WebSocket(RELAY_WS);
  ws.binaryType = 'arraybuffer';
  const events = []; // { ts: performance.now(), kind: 'binary'|'text', bytes?, text? }
  const state = { events, closed: null, error: null };
  ws.onmessage = (event) => {
    const ts = nowMs();
    if (typeof event.data !== 'string') {
      events.push({ ts, kind: 'binary', bytes: event.data.byteLength ?? 0 });
    } else {
      events.push({ ts, kind: 'text', text: event.data });
    }
  };
  ws.onclose = (event) => {
    state.closed = { ts: nowMs(), code: event.code, reason: event.reason || '(none)' };
  };
  ws.onerror = () => {
    state.error = state.error ?? 'ws error';
  };
  const opened = new Promise((resolve, reject) => {
    ws.onopen = () => resolve();
    const openErrorHandler = ws.onerror;
    ws.onerror = (e) => { openErrorHandler(e); reject(new Error(`cannot reach ${RELAY_WS}`)); };
    setTimeout(() => reject(new Error(`ws connect timeout: ${RELAY_WS}`)), CONNECT_TIMEOUT_MS);
  });
  return { ws, state, opened };
}

/** Preflight: fail loudly and immediately if the relay is not actually reachable, rather than
 * burning 40+ trial timeouts to discover that. The brief states the services are already up; this
 * is a defensive check, not an assumption that they might not be. */
async function preflightRelayReachable() {
  try {
    const { ws, opened } = openSession(`bargein-drive-preflight-${RUN_ID}`);
    await opened;
    ws.close();
    return true;
  } catch (err) {
    console.error(`PREFLIGHT FAIL: ${err.message}`);
    return false;
  }
}

// --- one trial: speak, interrupt, watch what actually stops -------------------------------------
//
// TIMING CONDITIONS — this is the single biggest empirical finding a cheap 2-sample pilot run of
// this exact script surfaced, and it reshaped this function, so it is documented here rather than
// only in a report someone might not read:
//
//   A pilot run that fired the interrupt on the first OBSERVED binary frame (the originally
//   planned design — "wait until playback is streaming, then interrupt") measured stop_latency of
//   -0.1ms and -0.0ms across 128- and 116-frame, ~400KB bursts. Not "fast" — NEGATIVE: the entire
//   burst had already finished arriving by the time this script's own event loop could react to
//   frame #1 and call ws.send() for the interrupt. On loopback, `main.rs`'s unpaced
//   `for chunk in audio_chunks { sink.send(...).await? }` loop (see this file's header) delivers
//   an entire multi-hundred-KB reply in under one JS event-loop tick. There is no "several hundred
//   milliseconds of streaming" window on this transport to land an interrupt inside of — that
//   premise, reasonable for a paced/chunked-over-time stream, is empirically false for this one.
//
//   The real bottleneck is `tts.synthesize()` itself: 2-3.4 SECONDS of synchronous, unyielding
//   HTTP wait (see header). So "immediate" fires the interrupt right after `speak`, while the
//   relay is provably still blocked inside that call — the only regime where "yield" could
//   possibly mean anything. "first_audio" (the original design) is kept as a smaller secondary
//   condition specifically to keep demonstrating the sub-millisecond-full-burst fact above with
//   fresh numbers on every run, not just this file's pilot.
//
//   TIMING itself was therefore not a knob that changed how fast the relay reacted — the pre-fix
//   read loop could not react to ANYTHING until the current turn's synthesize+send was fully done,
//   full stop. What TIMING changed was how much of that fixed, already-committed pipeline was
//   still ahead of the interrupt at the moment it was sent — which is why "immediate" measured
//   numbers close to a full turn's duration and "first_audio" measured numbers close to zero.
//   Neither number was "the relay's yield speed" in isolation; together they showed there
//   effectively wasn't one.
//
//   2026-08-28: that is now history, and both conditions are kept for exactly that reason — they
//   are the before/after pair. The relay reads control frames concurrently with an in-flight
//   synthesis, so "immediate" now lands where it was always aimed: inside the synthesis window,
//   which the relay abandons. It yields in ~1ms and delivers zero audio bytes. "first_audio" still
//   measures close to zero, but for the ORIGINAL reason — over loopback a whole burst is already
//   in the client's socket buffer before any interrupt can physically arrive — which is why it is
//   still not budget-graded. See printCannotSee() and the report's causal-check section.
//
/**
 * frameSpec: one of FRAME_TYPES[...] to interrupt with, or null for a control trial that never
 * interrupts at all (used to establish what "uninterrupted" looks like for the same utterance, so
 * the interrupted trials have something real to be compared against instead of an assumption).
 * timing: 'immediate' (send the interrupt IMMEDIATE_INTERRUPT_DELAY_MS after `speak`, while the
 *   relay is provably still blocked in synthesis — the primary, headline condition) or
 *   'first_audio' (send it on this script's first observed binary frame — the original design,
 *   kept to demonstrate the sub-ms-full-burst finding). Ignored for control trials.
 */
async function runTrial(frameSpec, trialIndex, timing = 'immediate') {
  const label = frameSpec ? frameSpec.type : 'control';
  const sessionId = `bargein-drive-${label}-${timing}-${trialIndex}-${RUN_ID}`;
  const { ws, state, opened } = openSession(sessionId);
  try {
    await opened;
  } catch (err) {
    return { frameType: label, trialIndex, sessionId, timing, ok: false, error: err.message };
  }

  ws.send(JSON.stringify({ type: 'start_listening', tenant_id: TENANT, session_id: sessionId }));
  await sleep(200);

  const speakSentAt = nowMs();
  ws.send(JSON.stringify({
    type: 'speak', tenant_id: TENANT, session_id: sessionId,
    text: UTTERANCE, voice_id: 'orb.warm.v1', emotion: 'warm',
  }));

  let interruptSentAt = null;
  if (frameSpec && timing === 'immediate') {
    // Fire while the relay is provably still blocked inside tts.synthesize() — see the block
    // comment above this function for why this, not first-audio, is the headline condition.
    await sleep(IMMEDIATE_INTERRUPT_DELAY_MS);
    interruptSentAt = nowMs();
    ws.send(JSON.stringify(frameSpec.payload(sessionId)));
  } else if (frameSpec) {
    // 'first_audio': wait for this script's own first observed binary frame, real Fish TTS
    // latency and all, then interrupt immediately. Bounded so a dead provider can't hang the run.
    const firstAudioDeadline = nowMs() + FIRST_AUDIO_TIMEOUT_MS;
    while (nowMs() < firstAudioDeadline && !state.closed) {
      if (state.events.some((e) => e.kind === 'binary')) break;
      await sleep(5);
    }
    if (state.events.some((e) => e.kind === 'binary')) {
      interruptSentAt = nowMs();
      ws.send(JSON.stringify(frameSpec.payload(sessionId)));
    }
    // If no audio ever showed up, fall through with interruptSentAt still null — handled by the
    // "zero audio" check below like any other no-audio trial.
  }

  // Keep collecting until quiet (no NEW frame for QUIET_WINDOW_MS) or a hard cap, so one hung
  // trial (or a provider that never responds) can never hang the whole run.
  //
  // TWO PHASES, DELIBERATELY NOT ONE: under 'immediate' timing the interrupt is sent long before
  // TTFB (real Fish TTS synthesis is 2-3.4 SECONDS per the pilot run — see runTrial's block
  // comment), so `state.events` is legitimately empty for a couple of seconds before anything
  // shows up at all. A single quiet-window check would (and, in an earlier version of this
  // script, actually did) mistake "nothing has arrived YET" for "the burst just ended" after only
  // QUIET_WINDOW_MS of initial silence, close the socket, and report a false "no audio ever
  // arrived" failure on every single trial — a bug in this harness, not a product finding.
  // `sawAnyEvent` gates the quiet-window check so the "waiting for the first frame" phase is
  // bounded only by the hard cap (generous: 20s vs. an observed ~2-3.4s TTFB), and the short
  // quiet-window logic only ever applies to "has the burst that's already started now ended."
  //
  // 2026-08-28: the gate below is on DELIVERY events, not on "any event". It used to be
  // `sawAnyEvent`, which was safe only because the pre-fix relay sent `speech_starting` AFTER
  // synthesis, so literally nothing arrived until the burst did. The fixed relay sends it BEFORE
  // synthesis — which is what protocol.rs:55-57 specifies it is for ("so the client can duck the
  // bed BEFORE audio arrives rather than after") — and that one early frame flipped `sawAnyEvent`
  // at t≈0, so the quiet window fired 1.2s later, mid-synthesis, and this script closed the socket
  // itself and then reported "no audio arrived" for the control, end_of_turn, and start_listening
  // suites. That was this harness mis-measuring a corrected relay, not the relay failing.
  const hardDeadline = nowMs() + TRIAL_HARD_TIMEOUT_MS;
  let lastCount = state.events.length;
  let lastChangeAt = nowMs();
  while (nowMs() < hardDeadline) {
    if (state.closed) break;
    await sleep(50);
    if (state.events.length !== lastCount) {
      lastCount = state.events.length;
      lastChangeAt = nowMs();
    } else if (deliveryUnderway(state.events) && nowMs() - lastChangeAt >= QUIET_WINDOW_MS) {
      break;
    }
  }
  try { ws.close(); } catch { /* already closed */ }
  await sleep(20);

  const binaryEvents = state.events.filter((e) => e.kind === 'binary');
  const ackTs = frameSpec?.yieldAckTypes && interruptSentAt !== null
    ? firstAckAfter(state.events, frameSpec.yieldAckTypes, interruptSentAt)
    : null;
  if (binaryEvents.length === 0) {
    // A trial that received no audio AT ALL is a broken pipeline in every case but one: an
    // interruption frame was sent, and the relay acknowledged yielding. That case is the strongest
    // possible result — the relay cancelled the utterance before a single byte left it — and
    // scoring it as an unusable sample (as this script did before 2026-08-28) would report a
    // correct relay as a failed measurement. The control trials still carry the fail-closed
    // "did the pipeline produce audio at all" check; see the global gate in main().
    if (ackTs !== null) {
      return {
        frameType: label, trialIndex, sessionId, timing, ok: true, speakSent: true,
        ttfbMs: null, totalBinaryFrames: 0, totalBinaryBytes: 0,
        framesAfterInterrupt: 0, bytesAfterInterrupt: 0,
        streamAlreadyEndedBeforeInterrupt: false,
        frameStopLatencyMs: null, // undefined by construction: no frame ever stopped, none was sent
        yieldLatencyMs: ackTs - interruptSentAt,
        yieldObservedVia: 'ack_only_no_audio_ever_sent',
        textTypeCounts: countTextTypes(state.events),
        closed: state.closed, wsError: state.error, utteranceChars: UTTERANCE.length,
      };
    }
    const reason = state.closed
      ? `socket closed before any audio: code=${state.closed.code} reason=${state.closed.reason}`
      : state.error
        ? `ws error before any audio: ${state.error}`
        : 'no binary audio frame arrived within timeout — relay/provider produced nothing';
    // speakSent: true — a `speak` frame WAS sent (and therefore billed) even though no audio came
    // back; only a connect failure (above) skips billing entirely. See printCostReport().
    return { frameType: label, trialIndex, sessionId, timing, ok: false, error: reason, speakSent: true };
  }
  const firstAudioEvent = binaryEvents[0];
  const ttfbMs = firstAudioEvent.ts - speakSentAt;
  const lastBinary = binaryEvents[binaryEvents.length - 1];
  const totalBinaryBytes = binaryEvents.reduce((s, e) => s + e.bytes, 0);
  const referenceTs = interruptSentAt ?? firstAudioEvent.ts;
  const framesAfter = binaryEvents.filter((e) => e.ts > referenceTs);
  const bytesAfter = framesAfter.reduce((s, e) => s + e.bytes, 0);

  const textTypeCounts = countTextTypes(state.events);

  const base = {
    frameType: label, trialIndex, sessionId, timing, ok: true, speakSent: true,
    ttfbMs, totalBinaryFrames: binaryEvents.length, totalBinaryBytes,
    textTypeCounts, closed: state.closed, wsError: state.error,
    utteranceChars: UTTERANCE.length,
  };
  if (!frameSpec || interruptSentAt === null) return base; // control, or interrupt never sent

  const frameStopLatencyMs = lastBinary.ts - interruptSentAt;
  const lastAfterTs = framesAfter.length ? framesAfter[framesAfter.length - 1].ts : null;
  // The graded number: the relay has yielded only when it will send no more audio for this turn
  // AND it has said so. Take the later of the two observables — a fast ack followed by more audio
  // is not a yield, and silence the client was never told about is not one either.
  let yieldLatencyMs = null;
  let yieldObservedVia = null;
  if (frameSpec?.yieldAckTypes) {
    if (ackTs !== null) {
      yieldLatencyMs = Math.max(ackTs, lastAfterTs ?? ackTs) - interruptSentAt;
      yieldObservedVia = lastAfterTs !== null && lastAfterTs > ackTs
        ? 'audio_after_ack' : 'ack';
    }
    // ackTs === null: the relay never acknowledged an interruption frame it documents as one.
    // Recorded as an unusable sample rather than silently graded on the audio timestamp alone.
  } else {
    // Non-interruption frame types keep the original transport-level definition; they have no
    // acknowledgement to wait for and are OBSERVE-only, never graded against the budget.
    yieldLatencyMs = frameStopLatencyMs;
    yieldObservedVia = 'last_audio_frame_no_ack_defined';
  }
  if (yieldLatencyMs === null) {
    return {
      ...base, ok: false,
      error: `relay never acknowledged the yield (expected ${frameSpec.yieldAckTypes.join('|')}`
        + ` after the interrupt); ${binaryEvents.length} audio frames arrived`,
    };
  }

  return {
    ...base,
    // Retained unchanged so the pre-2026-08-28 number is still on every report next to the new one.
    frameStopLatencyMs,
    yieldLatencyMs,
    yieldObservedVia,
    framesAfterInterrupt: framesAfter.length,
    bytesAfterInterrupt: bytesAfter,
    streamAlreadyEndedBeforeInterrupt: framesAfter.length === 0,
  };
}

async function runSuite(frameSpec, n, label, timing = 'immediate') {
  const results = [];
  for (let i = 1; i <= n; i++) {
    process.stdout.write(`  [${label} ${i}/${n}] `);
    const r = await runTrial(frameSpec, i, timing);
    if (r.ok) {
      const tail = frameSpec
        ? `ttfb=${fmt(r.ttfbMs)} frames=${r.totalBinaryFrames} yield=${fmt(r.yieldLatencyMs)}`
          + ` stop=${fmt(r.frameStopLatencyMs)} frames_after=${r.framesAfterInterrupt}`
          + `${r.streamAlreadyEndedBeforeInterrupt ? ' [STREAM_ALREADY_ENDED]' : ''}`
          + `${r.yieldObservedVia === 'ack_only_no_audio_ever_sent' ? ' [NO_AUDIO_EVER_SENT]' : ''}`
        : `ttfb=${fmt(r.ttfbMs)} frames=${r.totalBinaryFrames} bytes=${r.totalBinaryBytes}`;
      console.log(`ok  ${tail}`);
    } else {
      console.log(`FAIL  ${r.error}`);
    }
    results.push(r);
    await sleep(INTER_TRIAL_PAUSE_MS);
  }
  return results;
}

// --- optional, best-effort cross-check against the relay's own dev-logs -------------------------
// Reuses scripts/voice-ux-gate.mjs's exported readRows (the exact function G5 uses) via dynamic
// import so a failure here can never take down the core measurement above it.

async function crossCheckDevLogs(allTrials) {
  let readRows;
  try {
    ({ readRows } = await import('../scripts/voice-ux-gate.mjs'));
  } catch (err) {
    return { available: false, reason: `could not import scripts/voice-ux-gate.mjs: ${err.message}` };
  }
  let rows, files;
  try {
    ({ rows, files } = readRows(DEV_LOG_DIR));
  } catch (err) {
    return { available: false, reason: `could not read ${DEV_LOG_DIR}/: ${err.message}` };
  }
  const bySession = new Map();
  for (const r of rows) {
    if (r?.event !== 'frame.processed') continue;
    const sid = r?.in?.session_id;
    if (!sid) continue;
    if (!bySession.has(sid)) bySession.set(sid, []);
    bySession.get(sid).push(r);
  }
  const enriched = allTrials.map((t) => {
    const rowsForSession = bySession.get(t.sessionId) ?? [];
    const match = rowsForSession.find((r) => r?.in?.type === t.frameType);
    return {
      sessionId: t.sessionId,
      frameType: t.frameType,
      relaySideDispatchLatencyMs: match ? match.latency_ms ?? null : null,
      relaySideOutKind: match ? match.out?.kind ?? null : null,
      found: Boolean(match),
    };
  });
  return { available: true, dir: DEV_LOG_DIR, filesScanned: files.length, rowsScanned: rows.length, enriched };
}

// --- reporting ------------------------------------------------------------------------------

function printBanner() {
  console.log('='.repeat(88));
  console.log('BARGE-IN MEASUREMENT — relay-rs over its real WebSocket, real Fish TTS audio');
  console.log('='.repeat(88));
  console.log('Metric: relay-side frame-stop latency — transport-level.');
  console.log('This is NOT audio-yield-at-the-speaker. See "WHAT THIS MEASUREMENT CANNOT SEE" below.');
  console.log(`relay: ${RELAY_WS}   tenant: ${TENANT}   run: ${RUN_ID}`);
  console.log(`frame types: ${REQUESTED_FRAME_TYPES.join(', ')}   samples/type: ${SAMPLES_PER_TYPE}`
    + ` (floor ${MIN_USABLE_SAMPLES})   control samples: ${CONTROL_SAMPLES}`);
  console.log('Primary condition: interrupt fires ~50ms after `speak` (relay still blocked in TTS');
  console.log('synthesis). Secondary (barge_in only): interrupt fires on first observed audio frame —');
  console.log('kept to show the sub-ms-full-burst finding. See the block comment above runTrial() for why.');
  console.log('');
}

function printSampleTable(label, docComment, isBudgeted, results, minSamples = MIN_USABLE_SAMPLES) {
  console.log('-'.repeat(88));
  console.log(`${label}`);
  if (docComment) console.log(`  protocol.rs: ${docComment}`);
  console.log('-'.repeat(88));
  const header = '  #  session                 ttfb   frames   bytes   frames_after   yield_lat'
    + '   stop_lat  flag';
  console.log(header);
  for (const r of results) {
    if (!r.ok) {
      console.log(`  ${String(r.trialIndex).padStart(2)}  ${shortSid(r.sessionId).padEnd(22)}  FAIL: ${r.error}`);
      continue;
    }
    const flag = r.yieldObservedVia === 'ack_only_no_audio_ever_sent'
      ? 'NO_AUDIO_EVER_SENT'
      : r.streamAlreadyEndedBeforeInterrupt ? 'STREAM_ALREADY_ENDED' : '';
    console.log(
      `  ${String(r.trialIndex).padStart(2)}  ${shortSid(r.sessionId).padEnd(22)}  `
      + `${fmt(r.ttfbMs).padStart(7)}  ${String(r.totalBinaryFrames).padStart(6)}  `
      + `${String(r.totalBinaryBytes).padStart(7)}  ${String(r.framesAfterInterrupt).padStart(11)}  `
      + `${fmt(r.yieldLatencyMs).padStart(10)}  ${fmt(r.frameStopLatencyMs).padStart(9)}  ${flag}`,
    );
  }
  const usable = results.filter((r) => r.ok);
  const denom = usable.length;
  console.log(`  denominator: ${denom}/${results.length} usable`);
  if (denom < minSamples) {
    console.log(`  INSUFFICIENT: need >= ${minSamples} usable samples, got ${denom}.`);
    return { label, denom, attempted: results.length, sufficient: false };
  }
  // Graded on yieldLatencyMs (see this file's 2026-08-28 header note); frameStopLatencyMs is
  // still printed per sample above so the original number is never hidden.
  const latencies = usable.map((r) => r.yieldLatencyMs).sort((a, b) => a - b);
  const p50 = percentile(latencies, 0.5);
  const p95 = percentile(latencies, 0.95);
  const max = latencies[latencies.length - 1];
  const min = latencies[0];
  const alreadyEnded = usable.filter((r) => r.streamAlreadyEndedBeforeInterrupt).length;
  console.log(`  full usable set (n=${denom}), metric=yieldLatencyMs: min=${fmt(min)} p50=${fmt(p50)}`
    + ` p95=${fmt(p95)} max=${fmt(max)}`);
  const noAudio = usable.filter((r) => r.yieldObservedVia === 'ack_only_no_audio_ever_sent').length;
  console.log(`  NO_AUDIO_EVER_SENT (relay cancelled before one byte left it): ${noAudio}/${denom}`);
  console.log(`  STREAM_ALREADY_ENDED (interrupt sent after the burst had already fully drained): ${alreadyEnded}/${denom}`);
  let verdict = null;
  if (isBudgeted) {
    verdict = p95 <= BARGE_IN_YIELD_BUDGET_MS;
    console.log(`  budget: ${BARGE_IN_YIELD_BUDGET_MS}ms (backend/relay-rs/src/latency_budget.rs:18)`);
    console.log(`  VERDICT (p95 vs budget): ${verdict ? 'PASS' : 'FAIL'}`);
  } else {
    console.log('  no product-defined budget exists for this frame type (OBSERVE only, not graded)');
  }
  console.log('');
  return { label, denom, attempted: results.length, sufficient: true, p50, p95, max, min, alreadyEnded, verdict, latencies };
}

function printControlComparison(control, bargeInResults) {
  console.log('-'.repeat(88));
  console.log('CONTROL — same utterance, same protocol handshake, NO interrupt ever sent');
  console.log('-'.repeat(88));
  const usable = control.filter((r) => r.ok);
  for (const r of usable) {
    console.log(`  ${String(r.trialIndex).padStart(2)}  ${shortSid(r.sessionId).padEnd(22)}  `
      + `ttfb=${fmt(r.ttfbMs)}  frames=${r.totalBinaryFrames}  bytes=${r.totalBinaryBytes}`);
  }
  console.log(`  denominator: ${usable.length}/${control.length} usable`);
  if (usable.length === 0) {
    console.log('  no usable control samples — cannot compare.');
    console.log('');
    return null;
  }
  const avgControlBytes = usable.reduce((s, r) => s + r.totalBinaryBytes, 0) / usable.length;
  const avgControlFrames = usable.reduce((s, r) => s + r.totalBinaryFrames, 0) / usable.length;
  console.log(`  mean total bytes delivered (uninterrupted): ${avgControlBytes.toFixed(0)}`);
  console.log(`  mean total frames delivered (uninterrupted): ${avgControlFrames.toFixed(1)}`);

  const bargeUsable = bargeInResults.filter((r) => r.ok);
  if (bargeUsable.length > 0) {
    const avgBargeBytes = bargeUsable.reduce((s, r) => s + r.totalBinaryBytes, 0) / bargeUsable.length;
    const pct = avgControlBytes > 0 ? (100 * (1 - avgBargeBytes / avgControlBytes)) : 0;
    console.log(`  mean total bytes delivered (barge_in sent): ${avgBargeBytes.toFixed(0)}`);
    console.log(`  CAUSAL CHECK: barge_in reduced total bytes delivered by ${pct.toFixed(1)}% vs. sending`
      + ' nothing at all.');
    console.log(`  ${Math.abs(pct) < 15
      ? '  -> Effectively NO reduction. Consistent with the architecture finding above: the relay'
        + ' cannot read barge_in until the already-queued burst has fully drained, so barge_in did'
        + ' not truncate anything it did not already finish sending anyway.'
      : '  -> A real reduction was observed — barge_in measurably shortened delivery relative to'
        + ' the uninterrupted control.'}`);
  }
  console.log('');
  return { avgControlBytes, avgControlFrames };
}

function printDevLogsCrossCheck(crossCheck) {
  console.log('-'.repeat(88));
  console.log('BONUS: cross-check against the relay\'s own dev-logs/ (this is what G5 reads)');
  console.log('-'.repeat(88));
  if (!crossCheck.available) {
    console.log(`  not available: ${crossCheck.reason}`);
    console.log('');
    return;
  }
  console.log(`  scanned ${crossCheck.rowsScanned} rows across ${crossCheck.filesScanned} files in ${crossCheck.dir}/`);
  const found = crossCheck.enriched.filter((e) => e.found);
  console.log(`  matched ${found.length}/${crossCheck.enriched.length} trial sessions to a relay-side`
    + ' frame.processed row');
  if (found.length > 0) {
    const byType = {};
    for (const e of found) {
      (byType[e.frameType] ??= []).push(e.relaySideDispatchLatencyMs);
    }
    for (const [type, arr] of Object.entries(byType)) {
      const nums = arr.filter((n) => typeof n === 'number').sort((a, b) => a - b);
      if (nums.length === 0) continue;
      console.log(`  ${type}: relay-internal dispatch latency_ms (G5's own metric) `
        + `min=${nums[0].toFixed(3)} max=${nums[nums.length - 1].toFixed(3)} n=${nums.length}`
        + ' — this is the time the RUST MATCH-ARM itself took ONCE READ, not send-to-arrival.'
        + ' It will look fast even though the frame sat unread for the entire burst beforehand.');
    }
  }
  console.log('  NOTE: running this script grows dev-logs/relay-rs.ndjson\'s barge_in denominator');
  console.log('  from ~1 to 1 + (however many barge_in trials just ran) — re-run');
  console.log('  `node scripts/voice-ux-gate.mjs` after this to see G5 with a real sample.');
  console.log('');
}

function printCostReport(allAttemptedTrials) {
  console.log('-'.repeat(88));
  console.log('COST');
  console.log('-'.repeat(88));
  // Only trials that got past the WS-connect step actually sent a `speak` frame (and therefore
  // cost anything) — a connect failure never reaches that point. See runTrial's speakSent field.
  const speakCalls = allAttemptedTrials.filter((r) => r.speakSent).length
  const totalChars = speakCalls * UTTERANCE.length;
  const paise = (totalChars * ORB_RATE_PAISE_PER_1K_TTS_CHARS) / 1000;
  const inr = paise / 100;
  const usd = inr / INR_PER_USD;
  console.log(`  speak calls issued: ${speakCalls}  (utterance: ${UTTERANCE.length} chars each)`);
  console.log(`  total characters synthesized: ${totalChars}`);
  console.log(`  computed cost: ₹${inr.toFixed(2)} (~$${usd.toFixed(3)}) at `
    + `${ORB_RATE_PAISE_PER_1K_TTS_CHARS} paise/1000 chars `
    + '(backend/relay-py/src/orb_relay/app.py:229, ORB_RATE_PAISE_PER_1K_TTS_CHARS default)');
  console.log('  NOTE: computed from characters synthesized × this product\'s own configured');
  console.log('  rate — NOT read from a live cost-meter event. relay-rs reports usage to relay-py');
  console.log('  out-of-band (backend/relay-rs/src/main.rs:382-384) and this script does not verify');
  console.log('  that reporter actually fired, so this is the defensible number, not a telemetry read.');
  console.log('');
}

function printCannotSee() {
  console.log('='.repeat(88));
  console.log('WHAT THIS MEASUREMENT STRUCTURALLY CANNOT SEE');
  console.log('='.repeat(88));
  console.log(`
A real barge-in has three layers, and this script's number covers exactly the first one:

  (a) THE RELAY STOPS SENDING FRAMES  <- this script measures THIS, and only this.
      Client-observed wall clock, from send() of the interrupt to the LATER of the last binary
      frame for that turn and the relay's own yield acknowledgement. Real relay, real Fish TTS
      audio, real network hop. Still: a headless Node WebSocket client is not a phone, and "frame
      arrived at my socket" is not "the client did anything with it." A relay that has stopped
      sending is NECESSARY for barge-in to work; it is not sufficient.

  (b) THE CLIENT STOPS PLAYBACK  <- NOT measured here, at all.
      apps/mobile/src/voice/BargeIn.ts, RelayAudioPlayer.ts, and VoiceLoopController own this leg
      and are explicitly out of scope for this file (DO NOT EDIT list). Even if the relay stops
      sending immediately, a client with buffered/queued audio can keep playing what it already
      has. This script has no audio player and cannot observe that buffer at all.

  (c) THE SPEAKER GOES QUIET  <- NOT measured here, and not measured anywhere in this repo yet.
      audio-gate/README.md says this outright: "actual rendered TTS yield within 100 ms is not
      yet measured." That gate is the one built to eventually close this gap (mic/speaker capture,
      not logs) and this file deliberately does not duplicate or attempt it — out of scope by
      the brief, and it needs real audio hardware capture this script does not have.

OTHER LIMITS OF THIS SPECIFIC INSTRUMENT, worth naming rather than leaving implicit:

  - The interrupt is fired by this script directly as a control frame, on the FIRST observed audio
    frame. It bypasses the on-device VAD/AEC barge-in DETECTOR entirely (BARGE_IN_MIN_SPEECH_MS =
    200ms backchannel-vs-real-speech classifier, owned by apps/mobile/src/orb/, out of scope here).
    A real user's "yield" latency includes detection time BEFORE the frame this script sends would
    even be dispatched by the real app — this script's numbers are a floor, not a ceiling.
  - Localhost loopback only. No cellular/Wi-Fi RTT, no jitter a real device would add on top.
  - Two interrupt-timing conditions are run ('immediate': ~50ms after speak, while the relay is
    provably still inside TTS synthesis; 'first_audio': on this script's own first observed binary
    frame). Only 'immediate' is graded: 'first_audio' lands after loopback has already delivered
    the burst into the client's socket buffer, so a fast number there is an artifact of the
    transport, not evidence about the relay — see the block comment above runTrial(). Longer
    replies, and a second interrupt mid-reply, are not tested.
  - Zero audio delivered is scored as a PASS here when (and only when) an interruption frame was
    sent and acknowledged. That is the correct reading for a relay that cancels during synthesis,
    but it does mean this script cannot distinguish "cancelled correctly" from "TTS was broken for
    that one trial" on a per-trial basis. The uninterrupted control suite is what rules the second
    out, and it is why the control is not optional.
  - This number includes THIS SCRIPT'S OWN scheduling jitter, and the tail is dominated by it.
    interruptSentAt is stamped in Node just before ws.send(), and the ack's arrival time is stamped
    when Node's event loop gets around to the onmessage callback — so a GC pause or a late timer
    inflates the measurement without the relay being slow. Observed directly on 2026-08-28: a run
    whose client-observed p95 was 87.5ms had a relay-side p95 of 1.9ms and a max of 2.9ms across
    the same ten trials (dev-logs/relay-rs.ndjson, event "latency.barge_in_yield", which the relay
    stamps around its own read-to-yield window). Treat this script's number as an upper bound on
    the relay, and the relay-side event as the isolated one; they agree on the verdict, and the gap
    between them is Node, not the relay.
  - Fish TTS is a live third-party provider. Its own latency varies run to run; the percentiles
    below describe THIS run's sample, not a permanent guarantee.
  - This measures backend/relay-rs only. It does not exercise backend/relay-py, the gateway
    sidecar, or the mobile app's own state machine.
`);
}

// --- orchestration --------------------------------------------------------------------------

async function main() {
  const jsonOut = process.argv.includes('--json');
  printBanner();

  const reachable = await preflightRelayReachable();
  if (!reachable) {
    console.error(`FAIL: relay unreachable at ${RELAY_WS}. Nothing was measured. Exit 6.`);
    return 6;
  }

  const suites = {};       // primary condition: 'immediate' — interrupt ~50ms after `speak`
  const secondary = {};    // secondary condition: 'first_audio' — barge_in only (see runTrial docs)
  const allAttemptedTrials = [];

  for (const key of REQUESTED_FRAME_TYPES) {
    const spec = FRAME_TYPES[key];
    if (!spec) {
      console.error(`FAIL: unknown frame type "${key}" requested — not in protocol.rs's ClientFrame`
        + ` enum as mapped here. Known: ${Object.keys(FRAME_TYPES).join(', ')}. Exit 6.`);
      return 6;
    }
    console.log(`Running ${SAMPLES_PER_TYPE} trials for "${key}" [timing=immediate] …`);
    const results = await runSuite(spec, SAMPLES_PER_TYPE, key, 'immediate');
    allAttemptedTrials.push(...results);
    suites[key] = results;
    console.log('');
  }

  if (REQUESTED_FRAME_TYPES.includes('barge_in') && SECONDARY_FIRST_AUDIO_SAMPLES > 0) {
    console.log(`Running ${SECONDARY_FIRST_AUDIO_SAMPLES} secondary trials for "barge_in"`
      + ' [timing=first_audio] — demonstrates the sub-ms-full-burst finding, see runTrial docs …');
    const results = await runSuite(
      FRAME_TYPES.barge_in, SECONDARY_FIRST_AUDIO_SAMPLES, 'barge_in-first_audio', 'first_audio',
    );
    allAttemptedTrials.push(...results);
    secondary.barge_in_first_audio = results;
    console.log('');
  }

  console.log(`Running ${CONTROL_SAMPLES} control trials (no interrupt ever sent) …`);
  const control = await runSuite(null, CONTROL_SAMPLES, 'control');
  allAttemptedTrials.push(...control);
  console.log('');

  // Global fail-closed: zero audio, anywhere, ever.
  const everGotAudio = allAttemptedTrials.some((r) => r.ok && r.totalBinaryFrames > 0);
  if (!everGotAudio) {
    console.error('FAIL: zero binary audio frames arrived across every single trial. The relay, the');
    console.error('voice-provider-sidecar HTTP hop, or Fish TTS itself is not producing audio. This run');
    console.error('measured nothing about barge-in and must not be reported as a result. Exit 6.');
    return 6;
  }

  console.log('='.repeat(88));
  console.log('RESULTS — every sample, full distribution, denominators on everything');
  console.log('='.repeat(88));
  console.log('');

  const summaries = {};
  for (const key of REQUESTED_FRAME_TYPES) {
    const spec = FRAME_TYPES[key];
    summaries[key] = printSampleTable(
      `${key}  [timing=immediate]  (interruption documented in protocol.rs: ${spec.interruptionDocumented})`,
      spec.docComment,
      key === 'barge_in',
      suites[key],
    );
  }

  if (secondary.barge_in_first_audio) {
    console.log('SECONDARY CONDITION — demonstrates the sub-ms-full-burst finding, NOT graded against');
    console.log('the 100ms budget: this timing is chosen specifically to land AFTER the burst is nearly');
    console.log('or fully drained, so a "PASS" here would be an artifact of timing choice, not evidence');
    console.log('the relay yields quickly. See the block comment above runTrial() in the source.');
    printSampleTable(
      'barge_in  [timing=first_audio]  (secondary — see note above, NOT budget-graded)',
      FRAME_TYPES.barge_in.docComment,
      false,
      secondary.barge_in_first_audio,
      // Its own floor, not the primary MIN_USABLE_SAMPLES=10: this batch is deliberately smaller
      // (SECONDARY_FIRST_AUDIO_SAMPLES, default 5) and is supplementary, not the headline claim —
      // it should fail loudly only if IT under-delivered relative to what it set out to collect.
      SECONDARY_FIRST_AUDIO_SAMPLES,
    );
  }

  printControlComparison(control, suites.barge_in ?? []);

  const crossCheck = await crossCheckDevLogs(allAttemptedTrials.filter((r) => r.ok));
  printDevLogsCrossCheck(crossCheck);

  printCostReport(allAttemptedTrials);
  printCannotSee();

  // --- final verdict / exit code -----------------------------------------------------------
  console.log('='.repeat(88));
  console.log('VERDICT');
  console.log('='.repeat(88));
  let ok = true;
  const reasons = [];
  for (const [key, s] of Object.entries(summaries)) {
    if (!s.sufficient) {
      ok = false;
      reasons.push(`${key}: only ${s.denom}/${s.attempted} usable samples (< floor ${MIN_USABLE_SAMPLES})`);
    }
  }
  const bargeIn = summaries.barge_in;
  if (bargeIn && bargeIn.sufficient) {
    console.log(`barge_in yieldLatencyMs vs ${BARGE_IN_YIELD_BUDGET_MS}ms budget `
      + '(blueprints/ADHD-Focus-Orb-L8-Deep-Dive/03-VOICE-LATENCY-PIPELINE.md:167,'
      + ' backend/relay-rs/src/latency_budget.rs:18, AGENTS.md:88, CLAUDE.md:43):');
    console.log(`  n=${bargeIn.denom}  p50=${fmt(bargeIn.p50)}  p95=${fmt(bargeIn.p95)}  max=${fmt(bargeIn.max)}`);
    console.log(`  ${bargeIn.verdict ? 'PASS' : 'FAIL'}`);
    if (!bargeIn.verdict) {
      ok = false;
      reasons.push(`barge_in yield p95 (${fmt(bargeIn.p95)}) exceeds the ${BARGE_IN_YIELD_BUDGET_MS}ms budget`);
    }
  }
  if (jsonOut) {
    console.log('\n' + JSON.stringify({ runId: RUN_ID, summaries, crossCheckAvailable: crossCheck.available }));
  }
  console.log('');
  if (!ok) {
    console.log(`OVERALL: FAIL — ${reasons.join('; ')}`);
    return 6;
  }
  console.log('OVERALL: PASS');
  return 0;
}

// Guarded exactly like scripts/voice-ux-gate.mjs's own CLI entrypoint: only auto-run when this
// file is executed directly, never when imported — so a test file can import the pure functions
// below (percentile, fmt, envInt, shortSid) without that import itself spending Fish TTS credit,
// opening sockets, or running for minutes.
const isMain = process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href;
if (isMain) {
  main().then(
    (code) => process.exit(code),
    (error) => {
      console.error(`[barge-in-drive] UNCAUGHT ERROR ${error.stack || error.message}`);
      process.exit(1);
    },
  );
}

export {
  percentile, fmt, envInt, shortSid, UTTERANCE, BARGE_IN_YIELD_BUDGET_MS, MIN_USABLE_SAMPLES,
  deliveryUnderway, firstAckAfter, countTextTypes,
};
