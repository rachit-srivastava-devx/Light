/**
 * Measures the CLIENT half of barge-in: once the user has been detected as interrupting, how
 * much already-scheduled TTS audio does this app still play before it actually goes quiet?
 *
 * This complements the relay integration test that proves `barge_in` flips the generation's
 * provider cancel flag. It also covers what happens to audio the relay already delivered: queued
 * PCM may still be scheduled locally after the provider is retired. Whether a human can talk over
 * the orb therefore depends on both effects covered here: provider cancellation and retained-source
 * cancellation in `RelayAudioPlayer`.
 *
 * ## What the source does (grounded by reading, not assumed)
 *
 * `App.tsx` now has two production entry points. The fast path observes post-processing mic/VAD
 * frames while the orb is speaking, calls `VoiceLoopController.handleBargeInObservation`, and
 * cancels the relay/provider generation once speech is sustained. The final-transcript
 * `classifyConversationControl(text) === 'stop'` path remains a slower semantic fallback. Both
 * paths stop the retained local player and send the relay's `barge_in` cancellation frame. The
 * slower final-transcript fallback performs these actions in order:
 *
 *   1. `relayAudioPlayer.current?.handleClosing('user_pause')`   (RelayAudioPlayer.ts)
 *   2. `relayClientRef.current?.bargeIn()`                       (sends the `barge_in` socket frame only)
 *   3. `presenceEventsRef.current.cancelBy('user_speaks')`       (ambient presence cues, not TTS audio)
 *   4. `assistantSpeakingRef.current = false`                    (unblocks STT's echo filter)
 *   5. `devLogger.log('voice.stop_honoured', ...)`                (logs the *intent*, not an outcome)
 *
 * `RelayAudioPlayer.handleClosing('user_pause')` now actually cancels retained audio: every source
 * scheduled since the last stop/`completeTurn()` is kept in an instance-level list specifically so
 * this method has something to call `.stop()` on (`connectVoiceSource` itself never retains its
 * `source` argument — that capability, `AudioBufferSourceNodeLike.stop?()`, was previously used
 * exactly once in this codebase, on the ambient noise **bed** in `AudioGraph.ts`'s `shutdown()`,
 * never on a voice/TTS source). A stop also raises a suppression flag so chunks that keep arriving
 * after it — relay-rs sends a whole reply as one unpaced burst with no inter-chunk pacing, so the
 * remainder of a just-interrupted reply keeps landing in `enqueueChunk` for a while — are dropped
 * instead of scheduled. The flag is cleared by the next `completeTurn()`, which is the one signal
 * that reliably demarcates "the stopped turn is over" (relay-rs's synchronous synthesize-then-send
 * means a *new* turn's audio cannot start arriving before the old turn's `speech_complete`, and
 * therefore this player's `completeTurn()`, has already fired) — see "stop then new speak" below.
 *
 * `createBargeInCoordinator` owns the continuous-speech clock and dispatches cancellation
 * synchronously at the detection edge. `App.tsx` owns local playback because only the app has the
 * concrete `RelayAudioPlayer`; `VoiceLoopController` owns the provider cancellation transport.
 *
 * ## How this is measured
 *
 * No device audio exists or is needed (and the simulator mic is blocked by a host permission
 * anyway). This follows the same injected-clock pattern `RelayAudioPlayer.ts`'s own docstring
 * points at (`RelayAudioPlayer.test.ts` / `AudioGraph.test.ts` / `PresenceBoot.test.ts`): a fake
 * `RelayAudioContext` whose `currentTime` this suite drives by hand stands in for the audio clock,
 * and a `FakeSource` records every `.stop()` call so "was a buffer actually cancelled" is a
 * mechanical fact (a call count and its timestamp), never an inference from timing.
 *
 * Every scenario is graded against `BARGE_IN_YIELD_BUDGET_MS` (contracts.ts = 100), the same 100ms
 * restated in `backend/relay-rs/src/latency_budget.rs:18` and
 * `blueprints/ADHD-Focus-Orb-L8-Deep-Dive/03-VOICE-LATENCY-PIPELINE.md:167`, and every scenario is
 * paired with an identical do-nothing control (`requestStop: false`): the control must keep showing
 * today's pre-fix symptom (audio still playing, zero stop() calls) whenever there was something to
 * interrupt — that is what proves any difference in the "with stop" run was caused by the stop call,
 * not by some unrelated change to scheduling.
 *
 * ## What this file does NOT cover
 *
 * This proves what the client-side scheduling *state* does, deterministically. It cannot see, and
 * makes no claim about: whether the physical speaker actually goes quiet (that is a real-device
 * audio-graph/native-bridge property no fake context can observe — a `.stop()` call being invoked
 * is not the same fact as the speaker cone stopping moving); on-device VAD/AEC "the user started
 * talking" detection latency (upstream of everything here — this suite starts the clock at the
 * moment `handleClosing` is called, not at the moment a human opened their mouth); STT
 * finalisation + `classifyConversationControl` latency (the trigger-latency gap noted above); or
 * relay-side behaviour (layer 1, covered by the sibling measurement cited at the top of this file).
 * The provider-cancellation test at the bottom does cover the control effect, but not a physical
 * device's acoustic AEC or speaker cone.
 */

import { describe, expect, it } from 'vitest';

import {
  buildAudioGraph,
  type AudioBufferSourceNodeLike,
  type AudioNodeLike,
  type BiquadFilterNodeLike,
  type GainNodeLike,
} from '../presence/AudioGraph';
import { DEFAULT_AUDIO_GRAPH_CONFIG, type AudioBufferLike, type PresenceAudioContext } from '../presence/PresenceBoot';
import { createVoiceLoopController } from '../runtime/VoiceLoopController';
import { createBargeInCoordinator } from './BargeIn';
import { BARGE_IN_YIELD_BUDGET_MS } from './contracts';
import { RelayAudioPlayer, RELAY_AUDIO_SAMPLE_RATE_HZ } from './RelayAudioPlayer';

/* ---------------------------------------------------------------------------------------------
 * Fakes. Mirrors RelayAudioPlayer.test.ts's FakeContext/FakeGain/FakeSource (same shapes, same
 * injected-clock discipline) but adds the one hook that suite doesn't need: a `stopCalls` record
 * on the source, so this file can assert cancellation actually happens instead of assuming it.
 * Kept local rather than imported because RelayAudioPlayer.test.ts does not export its fakes.
 * `FakeBiquad` and a working `createBiquadFilter` (rather than a throw) exist only so the "bed
 * survives a voice stop" suite below can build a real bed via the real `buildAudioGraph` — no other
 * test in this file touches a filter node.
 * ------------------------------------------------------------------------------------------- */

class FakeParam {
  value = 0;
  setValueAtTime(): void {}
  setTargetAtTime(): void {}
  linearRampToValueAtTime(): void {}
}

class FakeNode implements AudioNodeLike {
  connectedTo: AudioNodeLike[] = [];
  connect(destination: AudioNodeLike): void {
    this.connectedTo.push(destination);
  }
}

class FakeGain extends FakeNode implements GainNodeLike {
  readonly gain = new FakeParam();
}

class FakeBiquad extends FakeNode implements BiquadFilterNodeLike {
  type = '';
  readonly frequency = new FakeParam();
  readonly Q = new FakeParam();
}

/** Records every scheduled start AND every stop() call, so "was this ever cancelled" is a fact. */
class FakeSource extends FakeNode implements AudioBufferSourceNodeLike {
  buffer: unknown = null;
  loop = false;
  startedAt: number[] = [];
  stopCalls: Array<number | undefined> = [];
  start(when: number): void {
    this.startedAt.push(when);
  }
  stop(when?: number): void {
    this.stopCalls.push(when);
  }
}

class FakeContext implements PresenceAudioContext {
  readonly destination = new FakeNode();
  currentTime = 0;
  sampleRate = RELAY_AUDIO_SAMPLE_RATE_HZ;
  createdSources: FakeSource[] = [];

  createBuffer(_channels: number, length: number): AudioBufferLike {
    const data = new Float32Array(length);
    return { getChannelData: () => data };
  }
  createBufferSource(): FakeSource {
    const source = new FakeSource();
    this.createdSources.push(source);
    return source;
  }
  createGain(): FakeGain {
    return new FakeGain();
  }
  createBiquadFilter(): FakeBiquad {
    return new FakeBiquad();
  }
}

/* ---------------------------------------------------------------------------------------------
 * Fixture + measurement helpers.
 * ------------------------------------------------------------------------------------------- */

const HZ = RELAY_AUDIO_SAMPLE_RATE_HZ; // 24_000 — ms*HZ/1000 is an integer for every integer ms.

/** `ms` milliseconds of voiced 16-bit PCM (alternating +/-8000, same shape as the sibling suite's
 *  `voicedPcm` helper) so `SpeechAudio.ts`'s `rejectNonSpeech` never mistakes it for padding. */
function voicedChunkMs(ms: number): ArrayBuffer {
  const sampleCount = (ms * HZ) / 1_000;
  const view = new Int16Array(sampleCount);
  for (let i = 0; i < sampleCount; i++) view[i] = i % 2 === 0 ? 8_000 : -8_000;
  return view.buffer;
}

interface Timeline {
  /** Total scheduled duration of the whole burst, ms. */
  readonly totalMs: number;
  /** `player.isPlaying()` sampled immediately after the (optional) stop request. */
  readonly playingRightAfterDecisionPoint: boolean;
  /** `player.isPlaying()` sampled 1ms before the burst's natural end. */
  readonly playingJustBeforeNaturalEnd: boolean;
  /** `player.isPlaying()` sampled exactly at the burst's natural end. */
  readonly playingAtNaturalEnd: boolean;
  /** How many AudioBufferSourceNodeLike instances this scenario created. */
  readonly sourceCount: number;
  /** How many of those sources ever received a stop() call, across the whole scenario. */
  readonly stopCallCount: number;
  /** The `when` (ms) argument of every stop() call, across every source. */
  readonly stopCallTimesMs: readonly number[];
}

/**
 * Delivers `chunkDurationsMs` as one unpaced burst at `ctx.currentTime = 0` — matching relay-rs's
 * own send loop, which pushes every chunk with no inter-chunk pacing (the sibling measurement's
 * finding) — then moves the audio clock to `decisionPointMs` and, only when `requestStop` is
 * true, calls the exact production entry point (`App.tsx`'s `handleClosing('user_pause')`)
 * there. Returns a timeline so a "with stop" run and a `requestStop: false` control can be
 * compared field-by-field.
 */
function runTimeline(
  chunkDurationsMs: readonly number[],
  decisionPointMs: number,
  requestStop: boolean,
  stopTwice = false,
): Timeline {
  const ctx = new FakeContext();
  const player = new RelayAudioPlayer(ctx, new FakeGain(), HZ);
  for (const ms of chunkDurationsMs) player.enqueueChunk(voicedChunkMs(ms));
  const totalMs = chunkDurationsMs.reduce((sum, ms) => sum + ms, 0);

  ctx.currentTime = decisionPointMs / 1_000;
  if (requestStop) {
    player.handleClosing('user_pause');
    if (stopTwice) player.handleClosing('user_pause');
  }
  const playingRightAfterDecisionPoint = player.isPlaying();

  const playingJustBeforeNaturalEnd = ((): boolean => {
    if (totalMs === 0) return false;
    ctx.currentTime = (totalMs - 1) / 1_000;
    return player.isPlaying();
  })();

  ctx.currentTime = totalMs / 1_000;
  const playingAtNaturalEnd = player.isPlaying();

  const stopCallTimesMs = ctx.createdSources
    .flatMap((source) => source.stopCalls)
    .map((when) => Math.round((when ?? 0) * 1_000));

  return {
    totalMs,
    playingRightAfterDecisionPoint,
    playingJustBeforeNaturalEnd,
    playingAtNaturalEnd,
    sourceCount: ctx.createdSources.length,
    stopCallCount: stopCallTimesMs.length,
    stopCallTimesMs,
  };
}

/** Of the scheduled chunks, how many still had audio at/after `stopAtMs` vs. had already fully
 *  finished — an independent, cursor-based cross-check on `playingRightAfterDecisionPoint`. */
function bufferRetentionCounts(
  chunkDurationsMs: readonly number[],
  stopAtMs: number,
): { readonly stillAudibleCount: number; readonly alreadyFinishedCount: number } {
  let cursor = 0;
  let stillAudibleCount = 0;
  let alreadyFinishedCount = 0;
  for (const ms of chunkDurationsMs) {
    cursor += ms;
    if (cursor > stopAtMs) stillAudibleCount += 1;
    else alreadyFinishedCount += 1;
  }
  return { stillAudibleCount, alreadyFinishedCount };
}

/* ---------------------------------------------------------------------------------------------
 * The scenario table. One source of truth: both the per-scenario tests and the fail-closed
 * completeness check at the bottom read this array's contents/length directly.
 * ------------------------------------------------------------------------------------------- */

interface RetainedAudioScenario {
  readonly name: string;
  readonly chunksMs: readonly number[];
  /** When, in ms of scheduled audio-time, the stop is requested (or "would be", for the control). */
  readonly stopAtMs: number;
}

const RETAINED_AUDIO_SCENARIOS: readonly RetainedAudioScenario[] = [
  { name: 'empty buffer — stop requested before any chunk was ever enqueued', chunksMs: [], stopAtMs: 0 },
  { name: 'single chunk — stop lands mid-chunk', chunksMs: [300], stopAtMs: 120 },
  {
    name: 'stop during the very first chunk of a 4-chunk burst',
    chunksMs: [250, 250, 250, 250],
    stopAtMs: 10,
  },
  {
    name: 'mid-burst — stop lands inside the third of four chunks',
    chunksMs: [250, 250, 250, 250],
    stopAtMs: 600,
  },
  {
    name: 'stop requested after playback had already finished',
    chunksMs: [250, 250, 250, 250],
    stopAtMs: 1_000,
  },
];

describe.each(RETAINED_AUDIO_SCENARIOS)('barge-in stop: $name', ({ chunksMs, stopAtMs }) => {
  const totalMs = chunksMs.reduce((sum, ms) => sum + ms, 0);
  const wouldHaveRetainedMs = Math.max(0, totalMs - stopAtMs); // what the pre-fix no-op left playing
  const { stillAudibleCount } = bufferRetentionCounts(chunksMs, stopAtMs);
  const hadAnyChunk = chunksMs.length > 0;

  it(`stop silences playback immediately (control still shows audio playing when there was any): ${totalMs}ms scheduled across ${chunksMs.length} buffer(s), stop at ${stopAtMs}ms`, () => {
    const withStop = runTimeline(chunksMs, stopAtMs, true);
    const control = runTimeline(chunksMs, stopAtMs, false);

    // The do-nothing control never calls handleClosing, so it must never call stop() either —
    // and, whenever anything was still scheduled to be audible, it must still be audible (the
    // pre-fix symptom, preserved deliberately so the comparison below is meaningful).
    expect(control.stopCallCount).toBe(0);
    expect(control.playingRightAfterDecisionPoint).toBe(stillAudibleCount > 0);

    // The fix: isPlaying() goes false immediately after a stop, regardless of how much was
    // scheduled — there is no window where retained audio is still reported as playing.
    expect(withStop.playingRightAfterDecisionPoint).toBe(false);
  });

  it(`stop calls .stop() on every currently-tracked source, at the stop instant: ${chunksMs.length} buffer(s), stop at ${stopAtMs}ms`, () => {
    const withStop = runTimeline(chunksMs, stopAtMs, true);

    if (hadAnyChunk) {
      // Every source scheduled so far gets a stop() call — including one whose own window had
      // already elapsed by the stop instant (harmless: same defensive style as AudioGraph's
      // `shutdown()` calling `bedSource.stop?.()` unconditionally) — there is no per-source
      // end-time bookkeeping to skip it, only a flat "stop everything still retained" sweep.
      expect(withStop.stopCallCount).toBe(chunksMs.length);
      for (const whenMs of withStop.stopCallTimesMs) expect(whenMs).toBe(stopAtMs);
    } else {
      expect(withStop.stopCallCount).toBe(0); // nothing was ever enqueued; nothing to stop
    }
  });

  const verdict = `PASS: 0ms retained is within the ${BARGE_IN_YIELD_BUDGET_MS}ms barge-in yield budget (pre-fix this scenario left ${wouldHaveRetainedMs}ms playing)`;
  it(`grades against BARGE_IN_YIELD_BUDGET_MS: ${verdict}`, () => {
    const withStop = runTimeline(chunksMs, stopAtMs, true);
    const measuredRetainedMs = withStop.playingRightAfterDecisionPoint ? wouldHaveRetainedMs : 0;
    expect(measuredRetainedMs).toBe(0); // stronger than the budget requires: fully eliminated, not merely under budget
    expect(measuredRetainedMs).toBeLessThanOrEqual(BARGE_IN_YIELD_BUDGET_MS);
  });
});

it(`walked ${RETAINED_AUDIO_SCENARIOS.length} timing-arrival scenarios (fail-closed denominator — extend this array, not a copy, to add a case)`, () => {
  expect(RETAINED_AUDIO_SCENARIOS).toHaveLength(5);
  expect(RETAINED_AUDIO_SCENARIOS.every((s) => Number.isFinite(s.stopAtMs))).toBe(true);
});

/* ---------------------------------------------------------------------------------------------
 * Idempotency: does calling handleClosing('user_pause') a second time do anything further?
 * ------------------------------------------------------------------------------------------- */

describe('stop is idempotent', () => {
  const chunksMs = [250, 250, 250, 250];
  const stopAtMs = 600; // mid third chunk, same as the table's "mid-burst" case

  it('calling handleClosing("user_pause") twice in a row is identical to calling it once', () => {
    const once = runTimeline(chunksMs, stopAtMs, true, false);
    const twice = runTimeline(chunksMs, stopAtMs, true, true);

    expect(twice).toEqual(once);
    expect(twice.stopCallCount).toBe(once.stopCallCount); // no extra stop() calls from the repeat
  });

  it('a stopped timeline is NOT the same as the do-nothing control — the stop has an observable effect', () => {
    // Before the fix this asserted `expect(twice).toEqual(control)`: stopping and never stopping
    // produced an identical timeline because handleClosing('user_pause') was a no-op. That
    // assertion encoded the bug as the spec. A real stop must diverge from doing nothing.
    const twice = runTimeline(chunksMs, stopAtMs, true, true);
    const control = runTimeline(chunksMs, stopAtMs, false);

    expect(twice).not.toEqual(control);
    expect(twice.stopCallCount).toBeGreaterThan(0);
    expect(control.stopCallCount).toBe(0);
  });

  it('does not throw on a repeated stop', () => {
    expect(() => runTimeline(chunksMs, stopAtMs, true, true)).not.toThrow();
  });
});

/* ---------------------------------------------------------------------------------------------
 * Suppression: chunks that arrive after handleClosing but before the stopped turn is closed out
 * by completeTurn() must be dropped, not scheduled — relay-rs delivers a whole reply as one
 * unpaced burst, so the remainder of a just-interrupted reply keeps landing in enqueueChunk for a
 * while. A fix that only cancels what is already scheduled and lets the next chunk through is not
 * a fix (this is the exact gap a plain "cancel the current source" patch would miss).
 * ------------------------------------------------------------------------------------------- */

describe('suppresses leftover chunks after a stop, before the stopped turn is closed', () => {
  it('a chunk that arrives after handleClosing but before completeTurn() is dropped, not scheduled', () => {
    const ctx = new FakeContext();
    const player = new RelayAudioPlayer(ctx, new FakeGain(), HZ);

    player.enqueueChunk(voicedChunkMs(250)); // chunk 1 of a burst: [0, 250)
    ctx.currentTime = 0.05;
    player.handleClosing('user_pause'); // user interrupts 50ms in

    // relay-rs already queued the rest of the burst before it learned about the stop — chunk 2
    // arrives anyway.
    player.enqueueChunk(voicedChunkMs(250)); // would-be chunk 2, arriving post-stop
    player.enqueueChunk(voicedChunkMs(250)); // and chunk 3, also post-stop

    expect(ctx.createdSources).toHaveLength(1); // only chunk 1's source was ever created
    expect(player.isPlaying()).toBe(false);
  });

  it('the same chunks schedule normally when no stop was ever requested (control)', () => {
    const ctx = new FakeContext();
    const player = new RelayAudioPlayer(ctx, new FakeGain(), HZ);

    player.enqueueChunk(voicedChunkMs(250));
    ctx.currentTime = 0.05;
    // No handleClosing call here — this is the control.
    player.enqueueChunk(voicedChunkMs(250));
    player.enqueueChunk(voicedChunkMs(250));

    expect(ctx.createdSources).toHaveLength(3);
  });
});

/* ---------------------------------------------------------------------------------------------
 * Stranding: a stop's suppression must not survive past the turn that was actually interrupted.
 * completeTurn() is ONE way a turn ends, but it has exactly one production caller (App.tsx's
 * onSpeechComplete) — a turn that ends any other way never reaches it. relay-rs closes the socket
 * on a provider failure via the exact same `Action::SendAndClose` path `session.rs`'s `fail()`
 * uses that a session pause uses (confirmed by reading `session.rs`/`main.rs`: both set
 * `Action::SendAndClose`, and `main.rs`'s `close_after` flag drives an identical `break
 * 'connection'`) — so a turn that ends in a provider failure will never get a `speech_complete`,
 * and therefore never reaches `completeTurn()`. A suppression flag cleared ONLY by completeTurn()
 * would then stay set forever, silently dropping every later chunk on this player instance — "no
 * reply, ever, until app restart," the single worst outcome for this product.
 *
 * `handleClosing('provider_failure')` is therefore ALSO a valid boundary: whichever reason reaches
 * this method, the turn it was scheduling audio for is conclusively over. `'user_pause'`
 * deliberately does NOT close out its OWN turn on the same call — that would let through the exact
 * leftover bytes it exists to suppress (see "suppresses leftover chunks" above). Only a SUBSEQUENT
 * boundary (completeTurn, or a provider_failure closing the connection) may lift it — implemented
 * as a turn epoch: `handleClosing('user_pause')` records which epoch it stopped without advancing
 * it; `completeTurn()` and `handleClosing('provider_failure')` both advance the epoch, and
 * `enqueueChunk` only suppresses while the current epoch still matches the stopped one.
 * ------------------------------------------------------------------------------------------- */

describe('a suppression does not survive past the turn that was actually interrupted', () => {
  it('stop mid-speech, then the turn ends via provider_failure (never completeTurn) — the next turn still plays', () => {
    const ctx = new FakeContext();
    const player = new RelayAudioPlayer(ctx, new FakeGain(), HZ);

    player.enqueueChunk(voicedChunkMs(300)); // old turn: [0, 300)
    ctx.currentTime = 0.1;
    player.handleClosing('user_pause'); // user interrupts — suppression armed for this turn

    // The turn ends in a provider failure, not normally — no completeTurn() call anywhere in this
    // test. If suppression were cleared only by completeTurn(), the next enqueueChunk below would
    // be wrongly dropped forever.
    player.handleClosing('provider_failure');

    ctx.currentTime = 0.2;
    player.enqueueChunk(voicedChunkMs(200)); // a genuinely new turn's first chunk

    expect(ctx.createdSources).toHaveLength(2); // [old (stopped), new (must have scheduled)]
    const [oldSource, newSource] = ctx.createdSources;
    expect(oldSource?.stopCalls).toEqual([0.1]); // the interrupted turn was still actually cut
    expect(newSource?.startedAt).toEqual([0.2]); // the new turn was NOT swallowed by the old stop
    expect(player.isPlaying()).toBe(true);
  });

  it('without any prior interrupt, a plain provider_failure between two turns never blocks the second (sanity)', () => {
    const ctx = new FakeContext();
    const player = new RelayAudioPlayer(ctx, new FakeGain(), HZ);

    player.enqueueChunk(voicedChunkMs(300));
    ctx.currentTime = 0.3;
    player.handleClosing('provider_failure'); // fails outright — never interrupted by the user

    ctx.currentTime = 0.4;
    player.enqueueChunk(voicedChunkMs(200));

    expect(ctx.createdSources).toHaveLength(2);
    expect(ctx.createdSources[1]?.startedAt).toEqual([0.4]);
  });

  it('stop, then completeTurn, then stop again — each interruption is independently effective', () => {
    const ctx = new FakeContext();
    const player = new RelayAudioPlayer(ctx, new FakeGain(), HZ);

    player.enqueueChunk(voicedChunkMs(250)); // turn 1: [0, 250)
    ctx.currentTime = 0.05;
    player.handleClosing('user_pause'); // interrupt turn 1
    player.completeTurn(); // turn 1 closes out normally (its speech_complete still arrives)

    ctx.currentTime = 0.1;
    player.enqueueChunk(voicedChunkMs(250)); // turn 2: starts fresh at "now"
    ctx.currentTime = 0.15;
    player.handleClosing('user_pause'); // interrupt turn 2 as well

    // A chunk arriving right after the second stop, before turn 2 is closed out, must still be
    // suppressed — the epoch must not have been "used up" by the first interruption.
    player.enqueueChunk(voicedChunkMs(250)); // leftover of turn 2, must be dropped

    const [turn1Source, turn2Source] = ctx.createdSources;
    expect(ctx.createdSources).toHaveLength(2); // the turn-2 leftover created no third source
    expect(turn1Source?.stopCalls).toEqual([0.05]);
    expect(turn2Source?.startedAt).toEqual([0.1]);
    expect(turn2Source?.stopCalls).toEqual([0.15]);
    expect(player.isPlaying()).toBe(false);
  });
});

/* ---------------------------------------------------------------------------------------------
 * Stop, then the next turn starts: does completeTurn() (fired later by the relay's eventual
 * speech_complete for the "stopped" turn) clear the suppression so a genuinely new reply plays
 * normally, without either swallowing the new reply or letting the old tail through?
 * ------------------------------------------------------------------------------------------- */

describe('stop then new speak', () => {
  it('a stop cancels the old chunk, and the new turn plays normally with no overlap', () => {
    const ctx = new FakeContext();
    const player = new RelayAudioPlayer(ctx, new FakeGain(), HZ);

    player.enqueueChunk(voicedChunkMs(300)); // old turn: scheduled [0ms, 300ms)
    ctx.currentTime = 0.1; // 100ms in: user says "stop" — 200ms of old audio was still ahead
    player.handleClosing('user_pause');
    // The relay eventually reports speech_complete for the OLD turn regardless (per the sibling
    // measurement this can take ~1.9s p50 to actually arrive after barge_in is sent); App.tsx's
    // onSpeechComplete handler calls completeTurn() unconditionally when it does. completeTurn()
    // is also what clears the post-stop suppression, so the chunk that follows is free to
    // schedule as a normal new turn.
    player.completeTurn();
    ctx.currentTime = 0.15; // the new reply's first chunk arrives 50ms later
    player.enqueueChunk(voicedChunkMs(200)); // new turn: scheduled [150ms, 350ms)

    const [oldSource, newSource] = ctx.createdSources;
    expect(ctx.createdSources).toHaveLength(2);
    expect(oldSource?.startedAt).toEqual([0]);
    expect(newSource?.startedAt).toEqual([0.15]); // starts at "now" — the new reply is not suppressed
    expect(oldSource?.stopCalls).toEqual([0.1]); // cancelled exactly at the stop instant — no more overlap

    // isPlaying() right after the new chunk is scheduled reflects only the new turn's own window
    // ([0.15, 0.35)), not a phantom tail of the old (stopped, 50ms earlier) one.
    expect(player.isPlaying()).toBe(true);
    ctx.currentTime = 0.35;
    expect(player.isPlaying()).toBe(false);
  });

  it('the same overlap happens even with no stop ever requested — completeTurn(), not handleClosing, disregards audio still in flight (control)', () => {
    const ctx = new FakeContext();
    const player = new RelayAudioPlayer(ctx, new FakeGain(), HZ);

    player.enqueueChunk(voicedChunkMs(300));
    ctx.currentTime = 0.1;
    // No handleClosing call here — this is the control for the test above.
    player.completeTurn();
    ctx.currentTime = 0.15;
    player.enqueueChunk(voicedChunkMs(200));

    const [oldSource, newSource] = ctx.createdSources;
    expect(newSource?.startedAt).toEqual([0.15]);
    expect(oldSource?.stopCalls).toHaveLength(0);
  });
});

/* ---------------------------------------------------------------------------------------------
 * INV1 (AGENTS.md / CLAUDE.md): the ambient bed is created once and survives everything except
 * the explicit pause/session-end contract. A voice stop must not be one of the things that can
 * touch it. Uses the real `buildAudioGraph` + `DEFAULT_AUDIO_GRAPH_CONFIG` (not a re-implemented
 * fake bed) over the same shared FakeContext, wired exactly as App.tsx wires them in production:
 * one context, one bed, and a RelayAudioPlayer that only ever receives the bed's `voiceGain`
 * branch — it never sees `bedSource`.
 * ------------------------------------------------------------------------------------------- */

describe('a voice stop does not touch the ambient bed', () => {
  it('handleClosing("user_pause") stops only the voice source it scheduled — bedSource never receives a stop() call', () => {
    const ctx = new FakeContext();
    const graph = buildAudioGraph(ctx, DEFAULT_AUDIO_GRAPH_CONFIG, null, 0);
    const player = new RelayAudioPlayer(ctx, graph.voiceGain, HZ);

    player.enqueueChunk(voicedChunkMs(300));
    ctx.currentTime = 0.1;
    player.handleClosing('user_pause');

    expect(ctx.createdSources).toHaveLength(2); // [bedSource, voiceSource]
    const [bedSource, voiceSource] = ctx.createdSources;
    expect(bedSource).toBe(graph.bedSource);

    expect(bedSource?.stopCalls).toHaveLength(0); // INV1: the bed survives a voice stop
    expect(voiceSource?.stopCalls).toEqual([0.1]); // the voice branch was actually cut
  });
});

describe('detected barge-in cancels provider work', () => {
  it('routes the detected edge through VoiceLoopController to the relay/provider cancellation port', () => {
    const providerAbort = new AbortController();
    const voiceLoop = createVoiceLoopController({} as never, {
      isClosed: false,
      startListening() {},
      sendAudio() {},
      endOfTurn() {},
      speak() {},
      bargeIn() { providerAbort.abort(); },
      pause() {},
    });

    voiceLoop.handleBargeInObservation({
      at_ms: 0,
      speech_probability: 0.95,
      orb_is_speaking: true,
      is_echo_residue: false,
    });
    expect(providerAbort.signal.aborted).toBe(false);

    expect(voiceLoop.handleBargeInObservation({
      at_ms: 200,
      speech_probability: 0.95,
      orb_is_speaking: true,
      is_echo_residue: false,
    })).toEqual([{ kind: 'barge_in_yield' }]);
    expect(providerAbort.signal.aborted).toBe(true);
  });

  it('aborts an in-flight provider call when sustained post-AEC speech is detected', async () => {
    const providerAbort = new AbortController();
    let cancelledAtMs: number | null = null;
    let cancellationBudgetMs: number | null = null;
    const providerCall = new Promise<never>((_resolve, reject) => {
      providerAbort.signal.addEventListener('abort', () => reject(new Error('provider_call_cancelled')), {
        once: true,
      });
    });
    const coordinator = createBargeInCoordinator({
      cancelProviderCall: (budgetMs) => {
        cancelledAtMs = 200;
        cancellationBudgetMs = budgetMs;
        providerAbort.abort();
      },
    });

    expect(
      coordinator.observe({
        at_ms: 0,
        speech_probability: 0.95,
        orb_is_speaking: true,
        is_echo_residue: false,
      }),
    ).toEqual({ yield: false, reason: 'too_brief' });
    expect(providerAbort.signal.aborted).toBe(false);

    const detectedAtMs = 200;
    expect(
      coordinator.observe({
        at_ms: detectedAtMs,
        speech_probability: 0.95,
        orb_is_speaking: true,
        is_echo_residue: false,
      }),
    ).toEqual({ yield: true });

    expect(providerAbort.signal.aborted).toBe(true);
    expect(cancellationBudgetMs).toBe(BARGE_IN_YIELD_BUDGET_MS);
    expect(cancelledAtMs).not.toBeNull();
    expect(cancelledAtMs! - detectedAtMs).toBeLessThanOrEqual(BARGE_IN_YIELD_BUDGET_MS);
    await expect(providerCall).rejects.toThrow('provider_call_cancelled');
  });

  it('never fires cancelProviderCall more than once for one sustained speech episode (one-shot latch)', () => {
    // Mutation-testing gate (G4) follow-up: without this guard, continuous speech during a long
    // orb utterance would spam barge_in cancellations at the relay instead of yielding once.
    let cancelCount = 0;
    const coordinator = createBargeInCoordinator({
      cancelProviderCall: () => {
        cancelCount += 1;
      },
    });

    coordinator.observe({ at_ms: 0, speech_probability: 0.95, orb_is_speaking: true, is_echo_residue: false });
    coordinator.observe({ at_ms: 200, speech_probability: 0.95, orb_is_speaking: true, is_echo_residue: false });
    expect(cancelCount).toBe(1);

    coordinator.observe({ at_ms: 250, speech_probability: 0.95, orb_is_speaking: true, is_echo_residue: false });
    expect(cancelCount).toBe(1);
  });
});
