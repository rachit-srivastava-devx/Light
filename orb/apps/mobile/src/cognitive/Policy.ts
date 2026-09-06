/**
 * The intervention policy: `decide(beliefs, registers, session, history, now) -> PolicyDecision`.
 *
 * `docs/BUILD-DIGEST.md` §2. Pure function, exhaustively testable, zero LLM calls (§4) — a model
 * never decides whether the orb speaks.
 *
 * TWO LOGGED MISTAKES APPLY DIRECTLY HERE, both flagged in `docs/adr/LESSONS.md` as this file's
 * repeat risk:
 *
 *   L1 — no optional-context-with-default inputs. A veto that defaults to `false` is an
 *        intervention firing that should have been suppressed. Every input is required; there is
 *        no `?? 0` anywhere in this file.
 *   L3 — the two blocks have different semantics and must not be transcribed the same way.
 *        HARD VETOES are ordered and first-match-wins. CANDIDATE GUARDS are all evaluated, and
 *        the least intrusive that fired wins. Collapsing guards into an if/elif chain would make
 *        the intrusiveness ordering decorative.
 */

import {
  DYSREGULATED_INTERVENTION_SET,
  DYSREGULATED_VETO_THRESHOLD,
  INTERVENTION_BUDGET_PER_SESSION,
  INTERVENTION_LADDER,
  REFRACTORY_FOCUSED_MS,
  REFRACTORY_FOCUSED_THRESHOLD,
  REFRACTORY_MS,
  type BeliefVector,
  type Intervention,
  type InterventionRecord,
  type PolicyDecision,
  type PolicyReasonId,
  type PolicySeverity,
  type RegisterSet,
} from './contracts';
import type { EpochMs, SessionSnapshot } from '../session/contracts';

/** §2's guard thresholds, one named constant each. No bare numbers in the guard bodies. */
const INITIATING_ABOVE = 0.6;
const WORKING_MEMORY_BELOW = 0.3;
const OVERWHELMED_ABOVE = 0.7;
const UNDER_STIMULATED_ABOVE = 0.7;
const BLOCKED_ABOVE = 0.7;
const UNCERTAIN_ABOVE = 0.7;
const FATIGUED_ABOVE = 0.7;
const MIND_WANDERING_ABOVE = 0.7;
const HYPERFOCUS_ABOVE = 0.8;
const GOAL_DECAY_ABOVE = 0.6;
const DO_NO_HARM_FOCUSED_ABOVE = 0.9;
const DO_NO_HARM_CONFIDENCE_ABOVE = 0.6;
const DO_NO_HARM_MAX_BARRIER_BELOW = 0.7;
const EXECUTION_STALL_MS = 3 * 60_000;
const EXECUTION_STALL_CONFIDENCE_ABOVE = 0.5;
const HYPERFOCUS_BOUNDARY_ELAPSED_MS = 45 * 60_000;

/**
 * Everything the policy needs, all required (L1). `elapsed_ms` and `execution_stalled_ms` are
 * passed in rather than derived from a clock so replay is bit-identical (invariant 6).
 */
export interface PolicyInput {
  readonly beliefs: BeliefVector;
  readonly registers: RegisterSet;
  readonly session: SessionSnapshot;
  readonly history: readonly InterventionRecord[];
  readonly now: EpochMs;
  /** Time since the session started. */
  readonly session_elapsed_ms: number;
  /** Time since the user last showed progress on the current step. */
  readonly execution_stalled_ms: number;
  /** True on the turn a step was just completed (drives Celebrate). */
  readonly step_just_completed: boolean;
  /** Escalates past the refractory and budget vetoes. */
  readonly severity: PolicySeverity;
}

function intrusiveness(intervention: Intervention): number {
  return INTERVENTION_LADDER.indexOf(intervention);
}

function maxBarrier(registers: RegisterSet): number {
  return Math.max(...Object.values(registers.barrier));
}

/** Refractory widens while the user is focused (§2): interrupting flow costs more. */
export function refractoryWindowMs(registers: RegisterSet): number {
  return registers.attention.Focused > REFRACTORY_FOCUSED_THRESHOLD
    ? REFRACTORY_FOCUSED_MS
    : REFRACTORY_MS;
}

function lastInterventionAt(history: readonly InterventionRecord[]): EpochMs | null {
  if (history.length === 0) return null;
  // History is not assumed sorted — taking the max is cheap and removes a caller obligation that
  // nothing would have enforced.
  return history.reduce((latest, r) => (r.at_ts > latest ? r.at_ts : latest), history[0]!.at_ts);
}

/** A guard that fired: which intervention, and why. */
interface FiredGuard {
  readonly intervention: Intervention;
  readonly reason: PolicyReasonId;
}

/**
 * ALL guards are evaluated (L3) — this returns every one that fired, and the caller picks the
 * least intrusive. It deliberately does not short-circuit.
 */
export function evaluateGuards(input: PolicyInput): readonly FiredGuard[] {
  const { beliefs, registers, execution_stalled_ms, session_elapsed_ms, step_just_completed } = input;
  const { attention, barrier, goal } = registers;
  const fired: FiredGuard[] = [];

  if (attention.Initiating > INITIATING_ABOVE) {
    fired.push({ intervention: 'Suggest', reason: 'guard.initiating' });
  }
  if (beliefs.WorkingMemory.value < WORKING_MEMORY_BELOW) {
    fired.push({ intervention: 'Suggest', reason: 'guard.working_memory_low' });
  }
  if (barrier.Overwhelmed > OVERWHELMED_ABOVE) {
    fired.push({ intervention: 'Suggest', reason: 'guard.overwhelmed' });
  }
  if (barrier.UnderStimulated > UNDER_STIMULATED_ABOVE) {
    // The opposite fix to Overwhelmed (§2): add stimulus rather than shrink the step.
    fired.push({ intervention: 'Presence', reason: 'guard.under_stimulated' });
  }
  if (barrier.Blocked > BLOCKED_ABOVE || barrier.Uncertain > UNCERTAIN_ABOVE) {
    fired.push({ intervention: 'Clarify', reason: 'guard.blocked_or_uncertain' });
  }
  if (
    execution_stalled_ms > EXECUTION_STALL_MS &&
    beliefs.Execution.confidence > EXECUTION_STALL_CONFIDENCE_ABOVE
  ) {
    fired.push({ intervention: 'Clarify', reason: 'guard.execution_stalled' });
  }
  if (barrier.Fatigued > FATIGUED_ABOVE) {
    fired.push({ intervention: 'Pause', reason: 'guard.fatigued' });
  }
  if (attention.MindWandering > MIND_WANDERING_ABOVE) {
    fired.push({ intervention: 'Redirect', reason: 'guard.mind_wandering' });
  }
  if (attention.Hyperfocus > HYPERFOCUS_ABOVE && session_elapsed_ms > HYPERFOCUS_BOUNDARY_ELAPSED_MS) {
    fired.push({ intervention: 'Pause', reason: 'guard.hyperfocus_boundary' });
  }
  if (goal.Decay > GOAL_DECAY_ABOVE) {
    fired.push({ intervention: 'Clarify', reason: 'guard.goal_decay' });
  }
  if (step_just_completed) {
    fired.push({ intervention: 'Celebrate', reason: 'guard.step_completed' });
  }

  return fired;
}

function commit(
  intervention: Intervention,
  reason: PolicyReasonId,
  input: PolicyInput,
  budgetRemaining: number,
): PolicyDecision {
  const isObserve = intervention === 'Observe';
  return {
    intervention,
    severity: input.severity,
    reason,
    // Observing is not an intervention: it must not start a refractory window, or a quiet policy
    // would lock itself out of ever speaking.
    refractory_until_ts: isObserve ? input.now : input.now + refractoryWindowMs(input.registers),
    budget_remaining: isObserve ? budgetRemaining : budgetRemaining - 1,
  };
}

export function decide(input: PolicyInput): PolicyDecision {
  const { registers, session, history, now, severity } = input;
  const budgetRemaining = INTERVENTION_BUDGET_PER_SESSION - history.length;
  const critical = severity === 'critical';

  // ── 1. HARD VETOES — ordered, first match wins (L3) ────────────────────────
  // (a) Never push a dysregulated user. Even critical severity cannot override this one; it only
  //     narrows the ladder to {Pause, Escalate} (INV7).
  if (registers.barrier.Dysregulated > DYSREGULATED_VETO_THRESHOLD) {
    return commit(DYSREGULATED_INTERVENTION_SET[0], 'veto.dysregulated', input, budgetRemaining);
  }
  // (b) Session mechanics own the floor during intake/clarify.
  if (session.state === 'INTAKE' || session.state === 'CLARIFY') {
    return commit('Observe', 'veto.session_mechanics', input, budgetRemaining);
  }
  // (c) Refractory, (d) budget — both yield to critical severity (§2).
  const last = lastInterventionAt(history);
  if (!critical && last !== null && now - last < refractoryWindowMs(registers)) {
    return commit('Observe', 'veto.refractory', input, budgetRemaining);
  }
  if (!critical && budgetRemaining <= 0) {
    return commit('Observe', 'veto.budget_spent', input, budgetRemaining);
  }
  // (e) DO-NO-HARM: a confidently focused user with no strong barrier is left alone. This is the
  //     veto that makes the product tolerable — §2 puts the false-interrupt:missed-help cost at
  //     ~5:1, so silence is the right default when focus is confidently observed.
  if (
    registers.attention.Focused > DO_NO_HARM_FOCUSED_ABOVE &&
    input.beliefs.Attention.confidence > DO_NO_HARM_CONFIDENCE_ABOVE &&
    maxBarrier(registers) < DO_NO_HARM_MAX_BARRIER_BELOW
  ) {
    return commit('Observe', 'veto.do_no_harm', input, budgetRemaining);
  }

  // ── 2. CANDIDATE GUARDS — all evaluated, least intrusive that fired wins (L3) ──
  const fired = evaluateGuards(input);
  if (fired.length === 0) {
    return commit('Observe', 'guard.none_fired', input, budgetRemaining);
  }

  const winner = fired.reduce((best, candidate) =>
    intrusiveness(candidate.intervention) < intrusiveness(best.intervention) ? candidate : best,
  );

  // ── 3. RESOLVE + COMMIT ───────────────────────────────────────────────────
  return commit(winner.intervention, winner.reason, input, budgetRemaining);
}
