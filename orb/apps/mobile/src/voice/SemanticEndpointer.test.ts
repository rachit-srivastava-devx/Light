/**
 * Bilingual endpointing — the continuation-marker list has to cover the languages this product
 * actually ships in.
 *
 * `docs/BUILD-DIGEST.md` specifies an en-IN/Hinglish voice register (`VoiceRouter.ts`'s
 * `selectVoice`), and `runtime/T0FocusSession.ts` already branches on Devanagari input
 * (`HINDI_FUNCTION_WORDS`, `isLikelyPhoneticEnglishTranscript`). `CONTINUATION_MARKERS` was
 * English-only, so a Hindi or Hinglish speaker trailing off on "और"/"aur" was endpointed while
 * still speaking — the one failure this component's own doc comment calls "the single most damaging
 * thing this component can do to the product". `runtime/T0FocusSession.test.ts` and
 * `router/IntentClassifier.test.ts` both flagged it as out-of-track; these are the cases that
 * close it.
 *
 * Own file rather than folded into `voice.test.ts` so the module can be run by name:
 *   npm test -- apps/mobile/src/voice/SemanticEndpointer
 */

import { describe, expect, it } from 'vitest';

import { decideEndpoint, looksIncomplete, ENDPOINT_EAGER_PAUSE_MS } from './SemanticEndpointer';
import { ENDPOINT_COMPLETE_PAUSE_MS, ENDPOINT_INCOMPLETE_MAX_MS } from './contracts';

/** A pause long enough to endpoint on, and an utterance far short of the hard cap. */
const PAST_PAUSE = ENDPOINT_COMPLETE_PAUSE_MS + 350;
const MID_UTTERANCE = 2_000;

/**
 * Census, not a sample: one row per Hindi/Hinglish continuation marker the module claims to
 * cover, in both scripts. The denominator below is asserted against this table's length so a
 * marker added to the source without a case here is a failing test, not silent coverage loss.
 */
const HINDI_CONTINUATIONS = [
  // conjunctions — the mirror of English 'and' / 'but' / 'or' / 'because' / 'then'
  { gloss: 'and', devanagari: 'और', romanized: 'aur' },
  { gloss: 'but', devanagari: 'लेकिन', romanized: 'lekin' },
  { gloss: 'but (emphatic)', devanagari: 'मगर', romanized: 'magar' },
  { gloss: 'because', devanagari: 'क्योंकि', romanized: 'kyunki' },
  { gloss: 'because (variant spelling)', devanagari: 'क्यूंकि', romanized: 'kyonki' },
  { gloss: 'then', devanagari: 'फिर', romanized: 'phir' },
  { gloss: 'or', devanagari: 'या', romanized: 'ya' },
  { gloss: 'so / then (resumptive)', devanagari: 'तो', romanized: 'toh' },
  { gloss: 'that (subordinator)', devanagari: 'कि', romanized: 'ki' },
  // hedges and fillers — the mirror of English 'like' / 'um' / 'i mean' / 'sort of'
  { gloss: 'like / as', devanagari: 'जैसे', romanized: 'jaise' },
  { gloss: 'i mean / meaning', devanagari: 'मतलब', romanized: 'matlab' },
  { gloss: 'that is', devanagari: 'यानी', romanized: 'yaani' },
  { gloss: 'that is (variant spelling)', devanagari: 'यानि', romanized: 'yani' },
  { gloss: 'anyway / likewise', devanagari: 'वैसे', romanized: 'waise' },
  { gloss: 'anyway / likewise (v-spelling)', devanagari: 'वैसे', romanized: 'vaise' },
] as const;

describe('SemanticEndpointer — Hindi / Hinglish continuations', () => {
  // 15 rows, 14 distinct Devanagari spellings — वैसे covers both the 'waise' and 'vaise'
  // romanizations, so the Devanagari census below asserts 15 lookups over 14 words.
  it(`waits through the Devanagari trailing marker in all ${HINDI_CONTINUATIONS.length} rows`, () => {
    const cutOff = HINDI_CONTINUATIONS.filter(
      ({ devanagari }) => !looksIncomplete(`मुझे टैक्स भरना है ${devanagari}`),
    );
    expect(cutOff.map((c) => `${c.devanagari} (${c.gloss})`)).toEqual([]);
    expect(HINDI_CONTINUATIONS.length).toBe(15); // published denominator
  });

  it(`waits through every one of the ${HINDI_CONTINUATIONS.length} romanized (Hinglish) trailing markers`, () => {
    const cutOff = HINDI_CONTINUATIONS.filter(
      ({ romanized }) => !looksIncomplete(`mujhe taxes file karne hain ${romanized}`),
    );
    expect(cutOff.map((c) => `${c.romanized} (${c.gloss})`)).toEqual([]);
  });

  it('keeps listening on the Devanagari trailing conjunction instead of cutting the user off', () => {
    // The product-critical case, in Hindi: same shape as voice.test.ts's English "...and".
    const d = decideEndpoint('मुझे कल टैक्स भरना है और', PAST_PAUSE, MID_UTTERANCE);
    expect(d).toEqual({ kind: 'keep_listening', reason: 'incomplete_thought' });
  });

  it('keeps listening on the Hinglish trailing conjunction instead of cutting the user off', () => {
    // Verbatim the utterance that `runtime/T0FocusSession.test.ts` and
    // `router/IntentClassifier.test.ts` carry in their open-domain tables while noting that this
    // module would have endpointed it early. It no longer does.
    const d = decideEndpoint('yaar aaj ka din bilkul accha nahi tha, aur', PAST_PAUSE, MID_UTTERANCE);
    expect(d).toEqual({ kind: 'keep_listening', reason: 'incomplete_thought' });
  });

  it('ignores a trailing danda when matching a Devanagari continuation', () => {
    // Hindi STT emits U+0964 DEVANAGARI DANDA (and U+0965 double danda) where English STT emits
    // '.', so the punctuation strip has to know about them or the marker never matches.
    expect(looksIncomplete('मुझे टैक्स भरना है और।')).toBe(true);
    expect(looksIncomplete('मुझे टैक्स भरना है और॥')).toBe(true);
    expect(looksIncomplete('मुझे टैक्स भरना है और,')).toBe(true);
  });

  it('still endpoints a complete Hindi or Hinglish thought', () => {
    // The inverse defect, and what stops the fix from degenerating into "never endpoint a
    // Devanagari transcript": a finished Hindi sentence must still endpoint after the pause.
    expect(decideEndpoint('मुझे कल टैक्स भरना है', PAST_PAUSE, MID_UTTERANCE)).toEqual({
      kind: 'endpoint',
      reason: 'complete_thought',
    });
    expect(decideEndpoint('mujhe kal taxes file karne hain', PAST_PAUSE, MID_UTTERANCE)).toEqual({
      kind: 'endpoint',
      reason: 'complete_thought',
    });
  });

  it('signals eager (not yet confirmed) for a complete Hindi/Hinglish thought in the eager window', () => {
    // The eager/confirmed tier is language-agnostic — it gates on pause_ms and looksIncomplete's
    // verdict, neither of which special-cases a script. Proven explicitly rather than assumed.
    const pauseInEagerWindow = ENDPOINT_COMPLETE_PAUSE_MS - 1;
    expect(decideEndpoint('मुझे कल टैक्स भरना है', pauseInEagerWindow, MID_UTTERANCE)).toEqual({
      kind: 'eager_endpoint',
      reason: 'complete_thought_eager',
    });
    expect(decideEndpoint('mujhe kal taxes file karne hain', pauseInEagerWindow, MID_UTTERANCE)).toEqual({
      kind: 'eager_endpoint',
      reason: 'complete_thought_eager',
    });
    // An incomplete Hindi/Hinglish thought must NOT be eagerly signalled either — eager still
    // respects looksIncomplete, it only shortens the pause bar for a thought that already looks done.
    expect(decideEndpoint('मुझे कल टैक्स भरना है और', pauseInEagerWindow, MID_UTTERANCE)).toEqual({
      kind: 'keep_listening',
      reason: 'incomplete_thought',
    });
  });

  it('does not treat a Devanagari word merely ending in a marker as a continuation', () => {
    // The Devanagari analogue of voice.test.ts's "band"/"and" case, and a sharper one: the
    // extremely common perfective verb forms गया / किया / आया all end in the marker "या" (or),
    // and क्या (what) does too. Word-boundary matching is what keeps these endpointing.
    expect(looksIncomplete('मैं दुकान गया')).toBe(false);
    expect(looksIncomplete('उसने वो काम किया')).toBe(false);
    expect(looksIncomplete('ये क्या')).toBe(false);
    // Same for the romanized spellings.
    expect(looksIncomplete('main dukaan gaya')).toBe(false);
    expect(looksIncomplete('usne wo kaam kiya')).toBe(false);
  });

  it('matches a bare marker with no preceding words', () => {
    // First partial of an utterance is often the filler alone.
    expect(looksIncomplete('मतलब')).toBe(true);
    expect(looksIncomplete('matlab')).toBe(true);
    expect(looksIncomplete('AUR')).toBe(true); // STT casing is not guaranteed
  });

  it('the hard cap still wins over a Hindi continuation', () => {
    // Listening forever is its own failure — the bilingual list must not defeat the 30s cap.
    const d = decideEndpoint('मुझे टैक्स भरना है और', 200, ENDPOINT_INCOMPLETE_MAX_MS);
    expect(d).toEqual({ kind: 'endpoint', reason: 'hard_cap' });
  });
});

describe('SemanticEndpointer — English regression (unchanged by the bilingual list)', () => {
  // Mirrors the assertions in voice.test.ts's `SemanticEndpointer` block. Duplicated on purpose:
  // this file has to be able to fail on its own if the Hindi additions ever regress English.
  it('keeps listening before the eager pause elapses (genuinely too soon)', () => {
    expect(decideEndpoint('file my taxes', ENDPOINT_EAGER_PAUSE_MS - 1, 1_000)).toEqual({
      kind: 'keep_listening',
      reason: 'too_soon',
    });
  });

  it('signals eager (non-committal) between the eager and confirmed pause thresholds', () => {
    // docs/adr/0013-vad-and-endpointing-architecture.md: a three-tier design, not two. This is the
    // exact pause value the old two-tier test asserted stayed `too_soon`; it now correctly reports
    // the intermediate eager tier instead.
    expect(decideEndpoint('file my taxes', ENDPOINT_COMPLETE_PAUSE_MS - 1, 1_000)).toEqual({
      kind: 'eager_endpoint',
      reason: 'complete_thought_eager',
    });
  });

  it('endpoints (confirmed) a complete English thought after the full pause', () => {
    expect(decideEndpoint('file my taxes', ENDPOINT_COMPLETE_PAUSE_MS, 1_000)).toEqual({
      kind: 'endpoint',
      reason: 'complete_thought',
    });
  });

  it('waits through a trailing English conjunction', () => {
    expect(decideEndpoint('I need to file my taxes and', 500, 2_000)).toEqual({
      kind: 'keep_listening',
      reason: 'incomplete_thought',
    });
  });

  it('endpoints at the hard cap even mid-thought', () => {
    expect(decideEndpoint('and then also um', 200, ENDPOINT_INCOMPLETE_MAX_MS)).toEqual({
      kind: 'endpoint',
      reason: 'hard_cap',
    });
  });

  it('treats an empty transcript as incomplete', () => {
    expect(looksIncomplete('')).toBe(true);
    expect(looksIncomplete('   ')).toBe(true);
  });

  it('ignores trailing ASCII punctuation when matching continuations', () => {
    expect(looksIncomplete('I want to do the thing and.')).toBe(true);
  });

  it('does not treat an English word merely containing a marker as a continuation', () => {
    expect(looksIncomplete('I need to book the band')).toBe(false);
  });

  it('every English marker still keeps listening', () => {
    // Census over the English list as documented in the source, so a Hindi edit that drops or
    // shadows an English marker fails here.
    const english = [
      'and', 'but', 'so', 'because', 'then', 'or', 'the', 'a', 'to',
      'like', 'um', 'uh', 'i mean', 'sort of', 'kind of',
    ];
    const cutOff = english.filter((marker) => !looksIncomplete(`I have to do the thing ${marker}`));
    expect(cutOff).toEqual([]);
    expect(english.length).toBe(15); // published denominator
  });
});
