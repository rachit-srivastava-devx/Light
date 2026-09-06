/**
 * Cognitive plane contract — the 9-belief vector, the 4 registers, and the policy's decision shape.
 *
 * Source of truth: `docs/BUILD-DIGEST.md` §2 ("Belief vector", "Registers", "Policy"), §3 (the
 * calibration and behaviour gates) and §4 (belief updates and the policy are pure functions; an LLM
 * may contribute bounded evidence but never picks a state, a route or a tone).
 */

import type { EpochMs, SessionSnapshot } from '../session/contracts';

// ---------------------------------------------------------------------------
// Belief vector
// ---------------------------------------------------------------------------

/** §2 — the nine beliefs, in the digest's order. Mirrored by backend proxy/eval contracts. */
export type BeliefName =
  | 'Goal'
  | 'Context'
  | 'Attention'
  | 'Execution'
  | 'WorkingMemory'
  | 'Energy'
  | 'EmotionalLoad'
  | 'Trust'
  | 'NoveltyPull';

export interface BeliefState {
  /** Probability 0..1. Updates are applied in log-odds space (§2) and mapped back. */
  readonly value: number;
  /** 0..1. Rises with evidence reliability, decays on a per-belief half-life (§2). */
  readonly confidence: number;
  /** Drives the decay term; null until the belief has ever seen evidence. */
  readonly last_evidence_ts: EpochMs | null;
}

export type BeliefVector = Readonly<Record<BeliefName, BeliefState>>;

/** §2 — the 3-tier evidence cascade of `EvidenceTiers.ts`; only tier2 involves a model. */
export type EvidenceTier =
  /** Rules / arithmetic, ~0ms, ₹0, bit-exact. */
  | 'tier0'
  /** Cosine over the ~150-utterance exemplar bank, ~1–5ms. */
  | 'tier1'
  /** Small schema-locked LLM, only on ambiguity (top-2 margin <0.15), off the hot path. */
  | 'tier2';

/** §2 — `on evidence e = (belief, weight w, reliability r)`. */
export interface Evidence {
  readonly belief: BeliefName;
  readonly weight: number;
  /** 0..1. */
  readonly reliability: number;
  readonly tier: EvidenceTier;
}

/** §2/§4 — bounded LLM evidence: `|w·r| ≤ 0.8 logits` per event. Applies to `tier2` evidence only. */
export const MAX_TIER2_EVIDENCE_LOGITS = 0.8;
/** §2 — `conf ← min(1, conf + κ·r)`, κ ≈ 0.4. */
export const CONFIDENCE_GAIN_KAPPA = 0.4;
/** §2 — named half-lives: Attention 90s, Energy 15min. The rest are `BeliefModel.ts`'s to tune. */
export const ATTENTION_CONFIDENCE_HALF_LIFE_MS = 90_000;
export const ENERGY_CONFIDENCE_HALF_LIFE_MS = 900_000;

/** §2 — the two half-lives in the update rule: confidence decay (T½) and reversion to prior (T_rev). */
export interface BeliefDecayParams {
  readonly confidence_half_life_ms: number;
  readonly reversion_half_life_ms: number;
}
export type BeliefDecayTable = Readonly<Record<BeliefName, BeliefDecayParams>>;

// ---------------------------------------------------------------------------
// Registers (§2)
// ---------------------------------------------------------------------------

/**
 * R1 Goal — 7 states, exclusive (scores sum to 1). Resolved against blueprint doc 13 §2 (the digest
 * only named `Decay`); see docs/adr/0003-register-enumeration.md.
 *
 * `GoalRenegotiation` is distinct from `Decay`: a deliberate mid-session scope cut ("let's just do
 * the first bit") vs. losing the thread — collapsing them misreads a healthy scope-cut as decay and
 * fires the wrong guard (doc 13 §2 rationale).
 */
export type GoalState =
  | 'NoGoal'
  | 'Formation'
  | 'Renegotiation'
  | 'Commitment'
  | 'Maintenance'
  | 'Completion'
  | 'Decay';
export type GoalRegister = Readonly<Record<GoalState, number>>;

/**
 * R2 Attention — 7 states, exclusive (scores sum to 1). Resolved against blueprint doc 13 §2.
 *
 * `Initiating` (pre-engagement ramp — trying to start, not yet in) is policy-inverse of `Focused`:
 * maximum support during Initiating, maximum silence during Focused — do not collapse it into
 * `Exploring` (doc 13 §2 rationale).
 */
export type AttentionState =
  | 'Initiating'
  | 'Focused'
  | 'Exploring'
  | 'MindWandering'
  | 'Hyperfocus'
  | 'ExternalInterruption'
  | 'Recovering';
export type AttentionRegister = Readonly<Record<AttentionState, number>>;

/**
 * R3 Barrier — 8 flags, multi-label: each score is independent 0..1 and they do NOT sum to 1.
 * Resolved against blueprint doc 13 §2. A barrier vector is not one-hot — e.g. Fatigued(0.7) +
 * Avoiding(0.6) + Uncertain(0.4) simultaneously is normal and expected (doc 13 §2).
 *
 * `UnderStimulated` is the opposite barrier to `Overwhelmed` and needs the opposite guard
 * (Overwhelmed → shrink the step; UnderStimulated → raise the bed / tighten the timebox).
 */
export type BarrierFlag =
  | 'Blocked'
  | 'Uncertain'
  | 'Overwhelmed'
  | 'UnderStimulated'
  | 'Avoiding'
  | 'Waiting'
  | 'Fatigued'
  | 'Dysregulated';
export type BarrierRegister = Readonly<Record<BarrierFlag, number>>;

/** Register-coupling assertions (doc 13 §2) — not enforced at the type layer; `Policy.ts`/tests own these. */
export const REGISTER_COUPLING_ASSERTIONS = [
  // Redirect only makes sense once a goal exists to redirect toward.
  'Goal=NoGoal AND Intervention=Redirect is invalid: redirect requires Goal in {Commitment, Maintenance}',
  // A session can't be idle and mid-hyperfocus at once.
  'Session=IDLE_PRESENT AND Attention=Hyperfocus is invalid: no session is live',
  // Legal and expected, NOT a bug: hyperfocusing on the wrong thing while avoiding the real task.
  'Attention=Hyperfocus AND Barrier.Avoiding=high is a legal, meaningful combination',
] as const;

/**
 * R4 Intervention — 9 values, fully enumerated in §2 and ordered by intrusiveness. The tuple order
 * IS the intrusiveness scale (`Observe(0) … Escalate(8)`), so the policy's "least-intrusive-that-
 * fired wins" rule compares indices into this array rather than a separate lookup table.
 */
export const INTERVENTION_LADDER = [
  'Observe',
  'Presence',
  'Capture',
  'Celebrate',
  'Clarify',
  'Suggest',
  'Redirect',
  'Pause',
  'Escalate',
] as const;
export type Intervention = (typeof INTERVENTION_LADDER)[number];

/** INV7 (§2) — `Barrier.Dysregulated > 0.8` reduces the whole ladder to these two. */
export const DYSREGULATED_INTERVENTION_SET = ['Pause', 'Escalate'] as const satisfies readonly Intervention[];

export interface RegisterSet {
  readonly goal: GoalRegister;
  readonly attention: AttentionRegister;
  readonly barrier: BarrierRegister;
}

// ---------------------------------------------------------------------------
// Policy (§2 `decide(beliefs, session, history) → intervention`)
// ---------------------------------------------------------------------------

/** INV7 veto threshold — `Barrier.Dysregulated > 0.8`. */
export const DYSREGULATED_VETO_THRESHOLD = 0.8;
/** INV6 — refractory ≥90s, widened to 240s while `Focused > 0.7`. */
export const REFRACTORY_MS = 90_000;
export const REFRACTORY_FOCUSED_MS = 240_000;
export const REFRACTORY_FOCUSED_THRESHOLD = 0.7;
/** INV6 — integer budget, ≤6 interventions per session (AGENTS.md invariant 6: counts are integers). */
export const INTERVENTION_BUDGET_PER_SESSION = 6;
/** §2 guards — hysteresis: ≥0.1 gap between a guard's enter and exit threshold. */
export const HYSTERESIS_GAP = 0.1;

/**
 * §2 — only `CRITICAL` overrides the refractory and budget vetoes. Two values, because a third
 * would need a rule in the digest to say what it overrides.
 */
export type PolicySeverity = 'normal' | 'critical';

/**
 * Exactly one id per line of §2's veto and guard lists, in the digest's order. Recorded on every
 * decision so the policy-behaviour replay gate (§6) can assert *why* an intervention fired, not
 * just that one did.
 */
export type PolicyReasonId =
  // 1. HARD VETOES
  | 'veto.dysregulated'
  | 'veto.session_mechanics'
  | 'veto.refractory'
  | 'veto.budget_spent'
  | 'veto.do_no_harm'
  // 2. CANDIDATE GUARDS (all evaluated; least-intrusive-that-fired wins)
  | 'guard.initiating'
  | 'guard.working_memory_low'
  | 'guard.overwhelmed'
  | 'guard.under_stimulated'
  | 'guard.blocked_or_uncertain'
  | 'guard.execution_stalled'
  | 'guard.fatigued'
  | 'guard.mind_wandering'
  | 'guard.hyperfocus_boundary'
  | 'guard.goal_decay'
  | 'guard.step_completed'
  /** "nothing fires, session live → Presence or Observe". */
  | 'guard.none_fired';

/** §2 step 3 — "RESOLVE + COMMIT: start refractory, decrement budget, log". */
export interface PolicyDecision {
  readonly intervention: Intervention;
  readonly severity: PolicySeverity;
  /** The veto or guard that decided this. Exactly one — the resolve step picks a winner. */
  readonly reason: PolicyReasonId;
  /** When the next non-critical intervention becomes admissible (INV6). */
  readonly refractory_until_ts: EpochMs;
  /** Integer, 0..INTERVENTION_BUDGET_PER_SESSION, after this decision is committed. */
  readonly budget_remaining: number;
}

/** What the policy needs from history: enough to evaluate refractory and budget, nothing more. */
export interface InterventionRecord {
  readonly intervention: Intervention;
  readonly at_ts: EpochMs;
}

/**
 * §2/§4 — pure function, no wall-clock and no randomness inside (AGENTS.md invariant 6): `now` is
 * injected so belief replay and the policy-behaviour gate are bit-identical. Signature only;
 * `Policy.ts` owns the body.
 */
export type PolicyDecideFn = (
  beliefs: BeliefVector,
  registers: RegisterSet,
  session: SessionSnapshot,
  history: readonly InterventionRecord[],
  now: EpochMs,
) => PolicyDecision;
