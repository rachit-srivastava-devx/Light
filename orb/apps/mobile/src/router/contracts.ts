/**
 * Router contract — intent labels and the pure `route(state, intent, stuck_count) → action` surface.
 *
 * Source of truth: `docs/BUILD-DIGEST.md` §2 ("Router — pure function" + the intent label set) and
 * §4 (the router is one of the zero-LLM-decision components; a model may only classify an ambiguous
 * intent, never pick a route).
 */

import type { SessionState } from '../session/contracts';
import type { ResponseEnvelope } from '../lld/ResponseEnvelope';

/** §2 — `Intent labels: {done, next, stuck, pause, question, chitchat}`. Exactly these six. */
export type IntentLabel = 'done' | 'next' | 'stuck' | 'pause' | 'question' | 'chitchat';

/**
 * ORB-ACCEPTANCE-CONTRACT Track B, gate B4 — the chosen response mode, explicit and observable on
 * every envelope, never inferred by the caller. Exactly these three; a fourth value is a compile
 * error at every call site that constructs an envelope, not a silent default.
 *
 * `focus` — the task/step pipeline produced this turn (atomize, step advance, pause, reanchor,
 * clarify, or a repair of the input channel itself).
 * `converse` — an open-domain conversational reply (chitchat, venting, opinion, meta-question,
 * off-topic fact, or a bounded canned social reply).
 * `teach` — an open-domain reply classified as an explanation/teaching request
 * (`IntentClassifier.isTeachRequestUtterance`).
 * `build` — Speed-of-Thought P0 (docs/SPEED-OF-THOUGHT-P0-CONTRACT.md §1.3): the module-brief
 * decompose/clarify/freeze loop, not a task/step turn and not open-domain chat.
 *
 * Coordinated with Track C, which is adding the same concept to `backend/relay-py` in parallel:
 * match this field name (`mode`) and these literal values exactly; do not add a further value
 * here without a corresponding backend change (`proxy/schemas.py`'s `ResponseMode`).
 */
export type OrbMode = 'converse' | 'focus' | 'teach' | 'build';

/**
 * `ResponseEnvelope` (`lld/ResponseEnvelope.ts`) plus the explicit `mode` field. A superset type
 * rather than an edit to `ResponseEnvelope` itself: `lld/ResponseEnvelope.ts` is outside this
 * track's owned files while other agents are editing the repo concurrently (see the track report).
 * Every `ModedResponseEnvelope` IS a `ResponseEnvelope` (structurally assignable wherever a plain
 * `ResponseEnvelope` is expected), so existing consumers (`AppModel.ts`'s `screenModelFromEnvelope`,
 * etc.) keep working unchanged; only `T0FocusSession.ts` — which owns the runtime that produces
 * every envelope — needs to know about the extra field.
 *
 * `mode` is typed optional here ONLY so that pre-existing fakes in files outside this track's scope
 * (e.g. `AppController.test.ts`'s `fakeRuntime()`, which returns `AppModel.ts`'s plain
 * `ResponseEnvelope`-typed `IDLE_ENVELOPE` for every method) keep typechecking without this track
 * editing files it does not own. This does NOT weaken B4 in practice: `T0FocusSession.ts`'s own
 * `makeEnvelope` takes `mode` as a required parameter with no default (a missing argument at any of
 * its call sites is a compile error), so every real envelope this runtime ever produces has an
 * explicit, non-inferred `mode`. What optionality preserves is only the VALUE domain guarantee — a
 * fourth, illegal mode string is still a compile error wherever `mode` is present.
 */
export interface ModedResponseEnvelope extends ResponseEnvelope {
  readonly mode?: OrbMode;
}

/**
 * §2 router table. One member per row:
 *   `advance_step`        — intent ∈ {done, next}: advance, speak the next step from cache
 *   `pause_session`       — intent == pause
 *   `reanchor_templated`  — intent == stuck AND stuck_count < 2: templated re-anchor, cached audio
 *   `crew_reatomize`      — intent == stuck AND stuck_count >= 2
 *   `crew_atomize`        — state == INTAKE
 *   `fast_voice`          — question / chitchat, and the table's `else`
 *
 * Tags only: `route` receives just (state, intent, stuck_count), so it has no step text, step index
 * or task string to put in a payload — the step gate and the crew read those from session state.
 */
export type RouterAction =
  | 'advance_step'
  | 'pause_session'
  | 'reanchor_templated'
  | 'crew_reatomize'
  | 'fast_voice'
  | 'crew_atomize';

/**
 * §2's annotations on each row, as data rather than folklore: `deterministic` means no model call is
 * involved at all (§4), `filler_covered` means the action is dispatched async behind a filler so the
 * zero-think-time-silence rule (§3: audio within ≤300ms always) still holds.
 */
export const ROUTER_ACTION_DISPATCH: Readonly<
  Record<RouterAction, { readonly deterministic: boolean; readonly filler_covered: boolean }>
> = {
  advance_step: { deterministic: true, filler_covered: false },
  pause_session: { deterministic: true, filler_covered: false },
  reanchor_templated: { deterministic: true, filler_covered: false },
  crew_reatomize: { deterministic: false, filler_covered: true },
  crew_atomize: { deterministic: false, filler_covered: true },
  /** §2 marks this "hot path, rare" — capped length, no filler. */
  fast_voice: { deterministic: false, filler_covered: false },
};

/** §2 — `intent == stuck AND stuck_count >= 2` escalates from the templated re-anchor to the crew. */
export const STUCK_REATOMIZE_THRESHOLD = 2;

/**
 * §2/§4 — pure function: same inputs, same action, always. Signature only; `Router.ts` owns the
 * lookup table. `stuck_count` is an integer count of consecutive `stuck` intents on the current
 * step (AGENTS.md invariant 6).
 */
export type RouteFn = (state: SessionState, intent: IntentLabel, stuck_count: number) => RouterAction;
