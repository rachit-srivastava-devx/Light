/**
 * Rule-driven slot filling before atomizing (`docs/BUILD-DIGEST.md` §2, blueprint doc 12 §4:
 * "Atomizing straight off a vague brain-dump produces vague steps").
 *
 * Four slots — scope, first_context, blocker, time_box — filled by rules first. A model is asked
 * only to *phrase* a question, never to decide which slot is missing (§4). Hard cap: 2 questions,
 * because a third turns intake into an interrogation and the user abandons.
 *
 * Pure. The caller owns asking; this decides what to ask and when to stop.
 */

/** §2's four slots. */
export type ClarifySlot = 'scope' | 'first_context' | 'blocker' | 'time_box';

/** Doc 12 §4 — the hard cap. */
export const MAX_CLARIFY_QUESTIONS = 2;

export interface SlotState {
  readonly scope: string | null;
  readonly first_context: string | null;
  readonly blocker: string | null;
  readonly time_box: string | null;
}

export const EMPTY_SLOTS: SlotState = {
  scope: null,
  first_context: null,
  blocker: null,
  time_box: null,
};

export type ClarifyDecision =
  | { readonly kind: 'ask'; readonly slot: ClarifySlot }
  | { readonly kind: 'proceed'; readonly reason: 'slots_filled' | 'question_cap_reached' };

/**
 * Slot priority. `scope` first because every other slot's answer depends on how big the task is;
 * `first_context` second because it is what the first step will name. `blocker` and `time_box` are
 * useful but not blocking — a task can be atomized without them.
 *
 * Judgment call: only the first two are *required*. Requiring all four would hit the 2-question cap
 * on every vague task and then proceed anyway, making the cap the real behaviour and the slots
 * decorative.
 */
const REQUIRED_SLOTS: readonly ClarifySlot[] = ['scope', 'first_context'];

/** Words that indicate a task is already scoped — no scope question needed. */
const SCOPE_MARKERS = /\b(one|single|just|only|first|quick)\b/i;
/** A task naming a concrete place/tool already carries its first context. */
const CONTEXT_MARKERS = /\b(in|on|at|open|email|doc|portal|app|form|folder|inbox)\b/i;
const TIME_MARKERS = /\b(\d+\s*(min|minute|hour|hr)s?|by\s+\w+day|today|tonight|tomorrow)\b/i;
const BLOCKER_MARKERS = /\b(stuck|can't|cannot|waiting|blocked|need\s+\w+\s+first)\b/i;

/**
 * Extracts what the utterance already answers, so the orb never asks for something it was just
 * told. Rule-first (§4); no model call.
 */
export function extractSlots(utterance: string, existing: SlotState = EMPTY_SLOTS): SlotState {
  const match = (re: RegExp): string | null => {
    const m = utterance.match(re);
    return m === null ? null : m[0];
  };
  return {
    scope: existing.scope ?? match(SCOPE_MARKERS),
    first_context: existing.first_context ?? match(CONTEXT_MARKERS),
    blocker: existing.blocker ?? match(BLOCKER_MARKERS),
    time_box: existing.time_box ?? match(TIME_MARKERS),
  };
}

/**
 * @param slots what is known so far
 * @param questions_asked how many clarify questions have already been asked this intake
 */
export function decideClarify(slots: SlotState, questions_asked: number): ClarifyDecision {
  // The cap is checked first: past it, we atomize with what we have rather than asking again.
  // Proceeding on partial information beats interrogating someone who came here to start working.
  if (questions_asked >= MAX_CLARIFY_QUESTIONS) {
    return { kind: 'proceed', reason: 'question_cap_reached' };
  }
  for (const slot of REQUIRED_SLOTS) {
    if (slots[slot] === null) return { kind: 'ask', slot };
  }
  return { kind: 'proceed', reason: 'slots_filled' };
}
