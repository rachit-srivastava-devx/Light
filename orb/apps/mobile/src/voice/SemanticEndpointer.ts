/**
 * Decides when the user has finished a thought, from streaming STT partials.
 *
 * `docs/BUILD-DIGEST.md` §1/§3: two thresholds, not one silence timer — a complete thought
 * endpoints after a short pause (~150ms), an incomplete one is waited on up to a 30s hard cap.
 * This is worth 300–500ms against a plain silence timer (§3), which is most of the conversational
 * turn's headroom.
 *
 * Pure: no timers, no clock. The caller supplies `pause_ms`, so the same transcript sequence
 * replays identically (AGENTS.md invariant 6).
 */

import {
  ENDPOINT_COMPLETE_PAUSE_MS,
  ENDPOINT_INCOMPLETE_MAX_MS,
} from './contracts';

export type EndpointDecision =
  | { readonly kind: 'endpoint'; readonly reason: 'complete_thought' | 'hard_cap' }
  /**
   * Fires once `pause_ms` crosses `ENDPOINT_EAGER_PAUSE_MS` and the transcript already looks
   * complete, but before the confirmed `ENDPOINT_COMPLETE_PAUSE_MS` threshold. Non-committal: it
   * never ends the turn (only `'endpoint'` does) — it is a hint for a filler-covering consumer
   * (Track J's scheduler) to start preparing/covering audio before confirmation, the same shape as
   * Deepgram Flux's `eager_eot_threshold` vs `eot_threshold`. See `ENDPOINT_EAGER_PAUSE_MS`'s doc
   * comment and `docs/adr/0013-vad-and-endpointing-architecture.md` for the full contract.
   */
  | { readonly kind: 'eager_endpoint'; readonly reason: 'complete_thought_eager' }
  | { readonly kind: 'keep_listening'; readonly reason: 'too_soon' | 'incomplete_thought' };

/**
 * The eager half of the eager/confirmed pair. Endpointing latency is a published, CHOSEN tradeoff,
 * not a fixed cost: LiveKit's own turn-detector numbers put ~295ms of endpoint latency at a 10%
 * false-cutoff (mid-thought interruption) rate, and ~543ms at 5% — this product's confirmed
 * threshold (`ENDPOINT_COMPLETE_PAUSE_MS` = 150ms) is already more aggressive than either point on
 * that curve, which is exactly why `looksIncomplete`'s accuracy matters so much here: a false
 * "complete" verdict at 150ms has very little of the audio VAD's 800ms hangover left to save it
 * (see `VADGate.ts`). The eager threshold below is deliberately shorter still — firing eager is
 * cheap to get wrong (reversible: it never ends the turn), unlike confirming — and sits near the
 * low end of the modal human inter-speaker gap (~0-200ms, Stivers et al., PNAS 2009). Configurable
 * via `decideEndpoint`'s fourth parameter: a tuning dial, not a hardcoded constant a caller cannot
 * override. (LiveKit's own turn-detector *model* is not adopted here or anywhere in this product —
 * it ships under a license restricted to LiveKit Agents; only its published latency numbers are
 * used as a benchmark reference. Silero VAD (MIT) remains this track's VAD recommendation.)
 */
export const ENDPOINT_EAGER_PAUSE_MS = 60;

export interface EndpointConfig {
  readonly eagerPauseMs?: number;
  readonly completePauseMs?: number;
}

/**
 * Trailing words that mean "I am mid-sentence", so a pause after them is a thinking pause rather
 * than a finished thought. ADHD speech restarts and trails often (§6's corpus is stratified on
 * exactly this), so endpointing on a bare silence timer cuts people off mid-thought — the single
 * most damaging thing this component can do to the product.
 *
 * Judgment call: these lists are hand-written, not learned. §6's classifier-500 eval is what would
 * justify replacing them; until that exists, a short conservative list beats a confident model.
 * English half here; the Hindi/Hinglish half is below, merged into one check.
 */
const CONTINUATION_MARKERS_EN = [
  'and',
  'but',
  'so',
  'because',
  'then',
  'or',
  'the',
  'a',
  'to',
  'like',
  'um',
  'uh',
  'i mean',
  'sort of',
  'kind of',
];

/**
 * The same list for Hindi — in both scripts, because this product's STT returns either. A
 * Devanagari rendering ("और") and a romanized/Hinglish one ("aur") are the same utterance, and
 * `docs/BUILD-DIGEST.md`'s en-IN/Hinglish voice register (`VoiceRouter.ts`'s `selectVoice`) plus
 * `runtime/T0FocusSession.ts`'s Devanagari branch (`HINDI_FUNCTION_WORDS`) mean both reach here.
 * English-only markers meant a Hindi speaker trailing off on "और"/"aur" was endpointed while still
 * speaking — the exact failure the comment above calls the worst thing this component can do.
 *
 * Same judgment call as the English list: hand-written and conservative, not learned. Words that
 * collide with an English word are deliberately left out, since a false positive here costs a wait
 * and a false negative costs a cut-off:
 *   - 'par'  (पर, "but"/"on")  — collides with English "par"
 *   - 'tab'  (तब, "then")      — collides with "tab", common in a productivity app
 *   - 'fir'  (फिर, "then")     — collides with "fir"; the 'phir' spelling is kept
 * Hindi postpositions (को/में/से/का/की/के) are the structural analogue of English 'the'/'a'/'to'
 * and would be the next addition, but each needs a false-positive check against colloquial
 * sentence-final usage that §6's classifier-500 eval is what would provide.
 *
 * 'ya' (या, "or") is the one known-ambiguous entry, kept because Hinglish STT renders "or" as "ya"
 * far more often than an English speaker ends a task dictation on informal "ya" — and the cost of
 * being wrong is a bounded wait, not a cut-off.
 */
const CONTINUATION_MARKERS_HI = [
  // conjunctions — the mirror of 'and' / 'but' / 'because' / 'then' / 'or' / 'so'
  'और', 'aur',
  'लेकिन', 'lekin',
  'मगर', 'magar',
  'क्योंकि', 'kyunki',
  'क्यूंकि', 'kyonki',
  'फिर', 'phir',
  'या', 'ya',
  'तो', 'toh',
  'कि', 'ki',
  // hedges and fillers — the mirror of 'like' / 'um' / 'i mean' / 'sort of'
  'जैसे', 'jaise',
  'मतलब', 'matlab',
  'यानी', 'yaani',
  'यानि', 'yani',
  'वैसे', 'waise', 'vaise',
];

const CONTINUATION_MARKERS = [...CONTINUATION_MARKERS_EN, ...CONTINUATION_MARKERS_HI];

/**
 * True when the transcript's tail suggests the speaker intends to continue.
 *
 * Matching is word-boundary aware in both scripts: the leading space in `` ` ${marker}` `` is what
 * stops "band" matching 'and', and equally what stops the very common Devanagari perfectives
 * गया / किया / आया and क्या (all ending in the marker "या") from being read as a trailing "or".
 *
 * No Unicode normalization pass: every marker above is NFC/NFD-identical (verified — none contains
 * a nukta or a decomposable vowel sign), so a normalization step would cost a call on the STT hot
 * path and change nothing.
 */
export function looksIncomplete(transcript: string): boolean {
  // U+0964/U+0965 are the Devanagari danda and double danda — what Hindi STT emits where English
  // STT emits '.', so the strip has to know them or a Devanagari marker never matches.
  const trimmed = transcript.trim().toLowerCase().replace(/[.,!?।॥]+$/, '');
  if (trimmed.length === 0) return true;
  return CONTINUATION_MARKERS.some(
    (marker) => trimmed === marker || trimmed.endsWith(` ${marker}`),
  );
}

/**
 * @param transcript the latest partial
 * @param pause_ms silence since the last speech frame
 * @param utterance_ms total elapsed time in this utterance, for the hard cap
 */
export function decideEndpoint(
  transcript: string,
  pause_ms: number,
  utterance_ms: number,
  config?: EndpointConfig,
): EndpointDecision {
  // The hard cap wins over everything: at 30s we endpoint even mid-word, because listening
  // forever is its own failure (the user gets no response at all).
  if (utterance_ms >= ENDPOINT_INCOMPLETE_MAX_MS) {
    return { kind: 'endpoint', reason: 'hard_cap' };
  }
  const eagerPauseMs = config?.eagerPauseMs ?? ENDPOINT_EAGER_PAUSE_MS;
  const completePauseMs = config?.completePauseMs ?? ENDPOINT_COMPLETE_PAUSE_MS;
  if (pause_ms < eagerPauseMs) {
    return { kind: 'keep_listening', reason: 'too_soon' };
  }
  if (looksIncomplete(transcript)) {
    return { kind: 'keep_listening', reason: 'incomplete_thought' };
  }
  if (pause_ms < completePauseMs) {
    return { kind: 'eager_endpoint', reason: 'complete_thought_eager' };
  }
  return { kind: 'endpoint', reason: 'complete_thought' };
}
