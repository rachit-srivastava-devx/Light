/**
 * The atomizer schema — the single TypeScript home for it.
 *
 * WHY THIS FILE EXISTS: `docs/adr/LESSONS.md` L5 set an extraction deadline. The schema was
 * duplicated once (StepGate) on the "extract at the third consumer" rule, and BUILD-DIGEST §8
 * names three: the step gate, the context pack's reuse format, and the clarify protocol. Tier 5
 * added the second and third, so the copies collapse here rather than becoming three
 * hand-maintained versions of one contract.
 *
 * THE REMAINING MIRROR IS CROSS-LANGUAGE AND DELIBERATE. The canonical definition now lives in
 * `backend/relay-py/src/orb_relay/proxy/schemas.py` (pydantic), which is where validation actually
 * runs — the client never validates a model's output, it receives an already-validated list. This
 * file is the TypeScript *shape* of that contract, kept in sync by hand.
 *
 * The bounds below MUST match `schemas.py`'s constants. A drift shows up as the client accepting a
 * payload the server would have rejected. When a third language appears, generate both from one
 * JSON Schema rather than adding a third hand-maintained copy.
 */

/** Mirrors `schemas.py` STEPS_TOTAL_MIN / STEPS_TOTAL_MAX. */
export const STEPS_TOTAL_MIN = 1;
export const STEPS_TOTAL_MAX = 12;
/** Mirrors `schemas.py` STEP_TEXT_MAX_CHARS. */
export const STEP_TEXT_MAX_CHARS = 120;
/** Mirrors `schemas.py` EST_MIN_MIN / EST_MIN_MAX. */
export const EST_MIN_MIN = 1;
export const EST_MIN_MAX = 15;

export interface AtomizerStep {
  readonly step_text: string;
  readonly est_min: number;
  readonly done_signal: string;
}

export interface AtomizerOutput {
  readonly steps: readonly AtomizerStep[];
  readonly steps_total: number;
}
