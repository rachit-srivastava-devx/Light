/**
 * Router — the pure `route(state, intent, stuck_count) -> action` lookup table.
 *
 * Source of truth: `docs/BUILD-DIGEST.md` §2 ("Router — pure function") and `router/contracts.ts`
 * (`RouteFn`, `RouterAction`, `STUCK_REATOMIZE_THRESHOLD`). 100% deterministic, no I/O, no model
 * call — the router only ever picks a `RouterAction` label; it never itself talks to a model.
 *
 * The digest's table, verbatim:
 *
 *   intent ∈ {done, next, pause}            → advance_step / pause_session
 *   intent==stuck AND stuck_count<2         → reanchor_templated
 *   intent==stuck AND stuck_count>=2        → crew_reatomize
 *   intent==question OR chitchat            → fast_voice
 *   state==INTAKE                            → crew_atomize
 *   else                                     → fast_voice
 *
 * JUDGMENT CALL (flag for review): `IntentLabel` is a closed 6-member union (done, next, stuck,
 * pause, question, chitchat) and the four intent-based rows above already partition it completely.
 * Read as a strict top-to-bottom if/elif chain in the digest's listed order, `state==INTAKE` would
 * be unreachable dead code — every possible intent value gets caught by an earlier row regardless
 * of state. That can't be the intended behaviour: during INTAKE the utterance is a raw task
 * brain-dump, not a dialogue intent, so whatever the (arguably meaningless) intent classification
 * of that text happens to be, INTAKE must still dispatch the atomizer rather than, say,
 * mis-routing to `fast_voice` because the brain-dump text pattern-matched "question". This
 * implementation therefore checks `state==INTAKE` FIRST, before any intent-based row, so the
 * state check actually fires instead of being permanently shadowed. Every other state falls
 * through to the intent-based rows exactly as listed.
 */

import type { SessionState } from '../session/contracts';
import { STUCK_REATOMIZE_THRESHOLD } from './contracts';
import type { IntentLabel, RouteFn, RouterAction } from './contracts';

export const route: RouteFn = (
  state: SessionState,
  intent: IntentLabel,
  stuck_count: number,
): RouterAction => {
  // §2 row 5, promoted first — see the JUDGMENT CALL note above: without this, `state==INTAKE`
  // is unreachable dead code given the four intent rows below already partition IntentLabel.
  if (state === 'INTAKE') return 'crew_atomize';

  // §2 row 1 — `intent ∈ {done, next, pause}`.
  if (intent === 'done' || intent === 'next') return 'advance_step';
  if (intent === 'pause') return 'pause_session';

  // §2 row 2/3 — `intent==stuck`, split on `STUCK_REATOMIZE_THRESHOLD`.
  if (intent === 'stuck') {
    return stuck_count >= STUCK_REATOMIZE_THRESHOLD ? 'crew_reatomize' : 'reanchor_templated';
  }

  // §2 row 4 — `intent==question OR chitchat` — hot path, rare, capped length.
  if (intent === 'question' || intent === 'chitchat') return 'fast_voice';

  // §2 row 6 — `else`. Unreachable given the exhaustive IntentLabel union above; kept for defence
  // against a future intent label being added without this table being revisited.
  return 'fast_voice';
};
