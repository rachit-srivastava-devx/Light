/**
 * The client's measurement of "how long has the user been silent" — the `pause_ms` input
 * `SemanticEndpointer.decideEndpoint` needs and, until this module existed, never received.
 *
 * `App.tsx` held a hardcoded `pauseMs = 0` at the only call site, which meant `decideEndpoint`
 * short-circuited on `pause_ms < eagerPauseMs` before it could ever consult `looksIncomplete`:
 * the entire early-completion path — both tiers, and the bilingual continuation markers — was
 * unreachable in the running app outside the 30s hard cap, and `VADGate`'s 800ms audio hangover
 * was the only thing that ended a turn. These two functions are the measurement, split out of the
 * component so the cases that broke it are assertable rather than reasoned about: no frame yet,
 * an invalid frame, and a clock that ran backwards.
 *
 * Pure: `nowMs` is a parameter, never a `Date.now()` read (the same discipline `VADGate.ts` and
 * `SemanticEndpointer.ts` follow, and `docs/BUILD-DIGEST.md` §4's determinism invariant).
 */

import { VAD_SILENCE_PROBABILITY } from './contracts';

/**
 * True when a mic frame should refresh "the user was last heard now".
 *
 * The threshold is `VAD_SILENCE_PROBABILITY` (the hysteresis floor), NOT `VAD_SPEECH_PROBABILITY`
 * (the onset ceiling), because this must agree with what `VADGate`'s `listening` phase counts as
 * voice when it refreshes `last_voice_at_ms`. If the two disagreed, the semantic endpointer would
 * see silence the audio gate did not and would cut people off inside a sentence — with none of
 * the 800ms hangover left to save it.
 *
 * An out-of-range or non-finite probability is not voice: `VADGate.validFrame` ignores such a
 * frame outright and leaves `last_voice_at_ms` alone, so treating it as voice here would make the
 * two clocks drift apart on exactly the frames neither of them trusts.
 */
export function isVoiceFrame(speechProbability: number): boolean {
  if (!Number.isFinite(speechProbability)) return false;
  if (speechProbability < 0 || speechProbability > 1) return false;
  return speechProbability >= VAD_SILENCE_PROBABILITY;
}

/**
 * Silence elapsed since the last voice frame, in ms.
 *
 * Returns 0 — "no measurable pause", which `decideEndpoint` reads as `too_soon` and never
 * endpoints on — for every case where the measurement is absent or untrustworthy:
 *   - `null`: no voice frame observed in this turn (mic permission denied, capture not started,
 *     or a transcript that arrived before the first frame). 0 restores exactly the pre-wiring
 *     behaviour, leaving VAD's 800ms hangover as the only endpoint. Failing safe here matters
 *     more than failing fast: the wrong answer cuts a user off mid-sentence.
 *   - a non-finite input, which would otherwise propagate NaN into a threshold comparison where
 *     every comparison is false and the behaviour silently becomes "never endpoint".
 *   - a negative difference: `Date.now()` is not monotonic, and an NTP correction between the mic
 *     callback and the transcript handler can land the "last voice" timestamp in the future.
 */
export function pauseMsSince(lastVoiceAtMs: number | null, nowMs: number): number {
  if (lastVoiceAtMs === null) return 0;
  if (!Number.isFinite(lastVoiceAtMs) || !Number.isFinite(nowMs)) return 0;
  return Math.max(0, nowMs - lastVoiceAtMs);
}
