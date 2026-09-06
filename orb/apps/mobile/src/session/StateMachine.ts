/**
 * Session state machine — the 9-state deterministic FSM (Register 0).
 *
 * Source of truth: `docs/BUILD-DIGEST.md` §2 (FSM diagram + invariants) and
 * `docs/adr/0003-register-enumeration.md` (the CLARIFY edges, resolved).
 *
 * This file owns two things the frozen `contracts.ts` deliberately left as "StateMachine.ts's to
 * write": the `TransitionTable` *data* (every legal edge, including the `os_interrupt` entry
 * duplicated into every state's map per the contract's own instruction — the table is data, no
 * special-casing in the reducer) and the guard predicates + reducer that walk it.
 *
 * Determinism (AGENTS.md invariant 6 / BUILD-DIGEST §4): no wall-clock or randomness is read
 * inside this file. Every guard that needs "now" takes it as an explicit parameter from the
 * caller, so replay is bit-identical.
 */

import type {
  InvariantViolation,
  SessionEvent,
  SessionSnapshot,
  SessionState,
  SnapshotInvariantChecker,
  Transition,
  TransitionGuardId,
  TransitionTable,
} from './contracts';

// ---------------------------------------------------------------------------
// Guard predicates — pure functions over explicit parameters only.
// ---------------------------------------------------------------------------

/** `atomize_ready [steps schema-valid]` — the atomizer output already passed `validateAtomizerOutput`. */
export function steps_schema_valid(atomizer_output_valid: boolean): boolean {
  return atomizer_output_valid;
}

/** `spoken_complete [validated step exists, INV2]` — the step at `step_index` came from the validated list. */
export function validated_step_exists(step_index: number, steps_total: number): boolean {
  return step_index >= 1 && step_index <= steps_total;
}

/** `policy emits intervention [passes veto+refractory+budget]` — the policy already resolved this; StateMachine just gates on the verdict. */
export function policy_admits(policy_passed_veto_and_budget: boolean): boolean {
  return policy_passed_veto_and_budget;
}

/** `intent==done [explicit, INV3]` — completion is never inferred from silence or a timeout. */
export function explicit_done_intent(intent: string | null): boolean {
  return intent === 'done';
}

/** `STEP_DONE --[more steps?]-->` — step_index < steps_total. */
export function has_more_steps(step_index: number, steps_total: number): boolean {
  return step_index < steps_total;
}

/** `os_resume [bed recovered ≤300ms]` — see `presence/contracts.ts` TRANSIENT_RECOVERY_BUDGET_MS. */
export function bed_recovered(recovery_ms: number, budget_ms: number): boolean {
  return recovery_ms <= budget_ms;
}

/** `bed fade [last step done + confirm, INV4]` — session ends only on explicit intent + confirm. */
export function last_step_done_and_confirmed(
  step_index: number,
  steps_total: number,
  confirmed: boolean,
): boolean {
  return confirmed && step_index >= steps_total;
}

/** `transcript [clarify slots incomplete]` — `ClarifyProtocol.ts` can't fill all required slots from the utterance alone. */
export function clarify_slots_incomplete(slots_filled: number, slots_required: number): boolean {
  return slots_filled < slots_required;
}

/** `clarify_answered [all required slots filled or 2-question cap hit]` — doc 12 §4 hard caps at 2 questions. */
export function clarify_slots_resolved(
  slots_filled: number,
  slots_required: number,
  questions_asked: number,
  question_cap: number,
): boolean {
  return slots_filled >= slots_required || questions_asked >= question_cap;
}

// ---------------------------------------------------------------------------
// Transition table — the FSM diagram (§2) as data.
// ---------------------------------------------------------------------------

const OS_INTERRUPT_ENTRY: { readonly os_interrupt: Transition } = {
  os_interrupt: { kind: 'to', to: 'INTERRUPTED', guard: null },
};

const USER_PAUSE_ENTRY: { readonly user_pause: Transition } = {
  user_pause: { kind: 'to', to: 'INTERRUPTED', guard: null },
};

export const SESSION_TRANSITION_TABLE: TransitionTable = {
  IDLE_PRESENT: {
    tap: { kind: 'to', to: 'INTAKE', guard: null },
    ...USER_PAUSE_ENTRY,
    ...OS_INTERRUPT_ENTRY,
  },

  INTAKE: {
    // INTAKE --transcript--> INTAKE (stay; atomize dispatched, filler covers) — the ungated stay edge.
    transcript: { kind: 'to', to: 'INTAKE', guard: null },
    // ADR 0003: INTAKE --transcript [clarify_slots_incomplete]--> CLARIFY. Modelled as a distinct
    // event (`needs_clarification`) so the plain `transcript` stay-edge above and the
    // clarify-routing edge are two separate table entries rather than one event with two guards
    // fighting over the same key (TransitionTable's per-state map is keyed by event).
    needs_clarification: { kind: 'to', to: 'CLARIFY', guard: 'clarify_slots_incomplete' },
    // INTAKE keeps its direct atomize_ready-->STEP_PRESENT edge for the common case where
    // clarification isn't needed (ADR 0003, "must ALSO keep its direct edge").
    atomize_ready: { kind: 'to', to: 'STEP_PRESENT', guard: 'steps_schema_valid' },
    ...USER_PAUSE_ENTRY,
    ...OS_INTERRUPT_ENTRY,
  },

  CLARIFY: {
    // CLARIFY --clarify_answered [clarify_slots_resolved]--> stay-and-dispatch-atomize (ADR 0003):
    // same shape as INTAKE's transcript stay-edge — dispatch happens outside the FSM, the state
    // itself just stays put until atomize_ready fires next.
    clarify_answered: { kind: 'to', to: 'CLARIFY', guard: 'clarify_slots_resolved' },
    // CLARIFY --atomize_ready--> STEP_PRESENT (ADR 0003).
    atomize_ready: { kind: 'to', to: 'STEP_PRESENT', guard: 'steps_schema_valid' },
    ...USER_PAUSE_ENTRY,
    ...OS_INTERRUPT_ENTRY,
  },

  STEP_PRESENT: {
    new_task: { kind: 'to', to: 'INTAKE', guard: null },
    spoken_complete: { kind: 'to', to: 'WORKING', guard: 'validated_step_exists' },
    ...USER_PAUSE_ENTRY,
    ...OS_INTERRUPT_ENTRY,
  },

  WORKING: {
    new_task: { kind: 'to', to: 'INTAKE', guard: null },
    policy_intervention: { kind: 'to', to: 'CHECK_IN', guard: 'policy_admits' },
    // WORKING --intent==done [explicit, INV3]--> STEP_DONE. INV3 is enforced here, not just
    // documented: the only edge into STEP_DONE is gated on `explicit_done_intent`, so no other
    // event (timeout, silence, os_resume) can ever reach STEP_DONE.
    intent_done: { kind: 'to', to: 'STEP_DONE', guard: 'explicit_done_intent' },
    intent_next: { kind: 'to', to: 'STEP_PRESENT', guard: 'has_more_steps' },
    reatomize_ready: { kind: 'to', to: 'STEP_PRESENT', guard: 'steps_schema_valid' },
    ...USER_PAUSE_ENTRY,
    ...OS_INTERRUPT_ENTRY,
  },

  CHECK_IN: {
    new_task: { kind: 'to', to: 'INTAKE', guard: null },
    reply: { kind: 'to', to: 'WORKING', guard: null },
    ...USER_PAUSE_ENTRY,
    ...OS_INTERRUPT_ENTRY,
  },

  STEP_DONE: {
    // STEP_DONE --[more steps?]--> STEP_PRESENT : SESSION_DONE — the diagram's only branch edge.
    step_advance: {
      kind: 'branch',
      guard: 'has_more_steps',
      when_true: 'STEP_PRESENT',
      when_false: 'SESSION_DONE',
    },
    ...USER_PAUSE_ENTRY,
    ...OS_INTERRUPT_ENTRY,
  },

  INTERRUPTED: {
    // INTERRUPTED --os_resume [bed recovered ≤300ms]--> prior state. No os_interrupt entry here:
    // the contract's own comment says every state's map gets one *except INTERRUPTED*.
    os_resume: { kind: 'resume_prior', guard: 'bed_recovered' },
  },

  SESSION_DONE: {
    new_task: { kind: 'to', to: 'INTAKE', guard: null },
    // SESSION_DONE --bed fade [last step done + confirm, INV4]--> IDLE_PRESENT. INV4 is enforced
    // here: the only edge out of SESSION_DONE is gated on `last_step_done_and_confirmed`, so the
    // session can never end on anything but explicit confirm at the last step.
    bed_faded: { kind: 'to', to: 'IDLE_PRESENT', guard: 'last_step_done_and_confirmed' },
    ...USER_PAUSE_ENTRY,
    ...OS_INTERRUPT_ENTRY,
  },
};

// ---------------------------------------------------------------------------
// Guard evaluation context — the union of every parameter any guard above needs.
// ---------------------------------------------------------------------------

/**
 * Everything a transition's guard might need to evaluate, gathered from the caller (StepGate,
 * Policy, ClarifyProtocol, InterruptionHandler) at the moment an event is dispatched. Only the
 * fields relevant to the guard actually firing need to be populated; unused fields are ignored.
 */
export interface GuardContext {
  readonly atomizer_output_valid?: boolean;
  readonly step_index?: number;
  readonly steps_total?: number;
  readonly policy_passed_veto_and_budget?: boolean;
  readonly intent?: string | null;
  readonly recovery_ms?: number;
  readonly recovery_budget_ms?: number;
  readonly confirmed?: boolean;
  readonly slots_filled?: number;
  readonly slots_required?: number;
  readonly questions_asked?: number;
  readonly question_cap?: number;
  /** Only consulted by the `resume_prior` transition shape (`INTERRUPTED --os_resume--> prior state`). */
  readonly prior_state?: SessionState;
}

/** Caller-suggested resume state disagrees with the FSM snapshot that recorded the interruption. */
export class InvalidResumeStateError extends Error {
  constructor(recorded: SessionState | null, supplied: SessionState | undefined) {
    super(
      `StateMachine: resume target must match interrupted_from history (recorded ${recorded ?? 'null'}, supplied ${supplied ?? 'undefined'})`,
    );
    this.name = 'InvalidResumeStateError';
  }
}

/**
 * Thrown when a `branch` edge is dispatched without the context its guard needs.
 *
 * A `to` edge whose guard evaluates false stays put. A `branch` edge has no such default — it
 * commits to `when_true` or `when_false` either way, so a `?? 0` fallback would
 * silently pick a target from data the caller never supplied. For the FSM's only branch
 * (`STEP_DONE --[more steps?]-->`) that target is `SESSION_DONE`: an empty context would end the
 * session and let the bed go silent (INV4, and INV1 via `BedOptionalState`). Fail loudly instead.
 */
export class MissingGuardContextError extends Error {
  constructor(guard: TransitionGuardId, detail: string) {
    super(`StateMachine: branch guard '${guard}' dispatched without required context — ${detail}`);
    this.name = 'MissingGuardContextError';
  }
}

/**
 * Assert a branch guard's inputs are actually present. Unlisted guards throw by default: a future
 * branch edge must opt in here deliberately rather than inherit a silent `?? 0`.
 */
function assertBranchContext(guard: TransitionGuardId, ctx: GuardContext): void {
  if (guard === 'has_more_steps') {
    if (!Number.isInteger(ctx.step_index) || !Number.isInteger(ctx.steps_total)) {
      throw new MissingGuardContextError(
        guard,
        `step_index and steps_total must both be supplied as integers (got ${ctx.step_index}, ${ctx.steps_total})`,
      );
    }
    return;
  }
  throw new MissingGuardContextError(guard, 'no required-context rule declared for this branch guard');
}

function evaluateGuard(guard: TransitionGuardId | null, ctx: GuardContext): boolean {
  switch (guard) {
    case null:
      return true;
    case 'steps_schema_valid':
      return steps_schema_valid(ctx.atomizer_output_valid ?? false);
    case 'validated_step_exists':
      return validated_step_exists(ctx.step_index ?? 0, ctx.steps_total ?? 0);
    case 'policy_admits':
      return policy_admits(ctx.policy_passed_veto_and_budget ?? false);
    case 'explicit_done_intent':
      return explicit_done_intent(ctx.intent ?? null);
    case 'has_more_steps':
      return has_more_steps(ctx.step_index ?? 0, ctx.steps_total ?? 0);
    case 'bed_recovered':
      return bed_recovered(ctx.recovery_ms ?? Number.POSITIVE_INFINITY, ctx.recovery_budget_ms ?? 0);
    case 'last_step_done_and_confirmed':
      return last_step_done_and_confirmed(ctx.step_index ?? 0, ctx.steps_total ?? 0, ctx.confirmed ?? false);
    case 'clarify_slots_incomplete':
      return clarify_slots_incomplete(ctx.slots_filled ?? 0, ctx.slots_required ?? 0);
    case 'clarify_slots_resolved':
      return clarify_slots_resolved(
        ctx.slots_filled ?? 0,
        ctx.slots_required ?? 0,
        ctx.questions_asked ?? 0,
        ctx.question_cap ?? 0,
      );
    default: {
      const _exhaustive: never = guard;
      return _exhaustive;
    }
  }
}

// ---------------------------------------------------------------------------
// Reducer
// ---------------------------------------------------------------------------

/**
 * `dispatch(state, event, ctx) -> nextState`. An absent (state, event) pair is a no-op per
 * `contracts.ts`'s documented behaviour: "an absent entry means ignore the event" (§4: no
 * inference, no fallback state guessing). A guard that evaluates false is also a no-op — the
 * event is legal in shape but its precondition didn't hold, so the FSM stays put rather than
 * guessing a state.
 */
export function dispatch(
  current: SessionState | SessionSnapshot,
  event: SessionEvent,
  ctx: GuardContext = {},
): SessionState {
  const currentState = typeof current === 'string' ? current : current.state;
  const transition = SESSION_TRANSITION_TABLE[currentState][event];
  if (!transition) return currentState;

  switch (transition.kind) {
    case 'to': {
      if (transition.guard !== null && !evaluateGuard(transition.guard, ctx)) return currentState;
      return transition.to;
    }
    case 'branch': {
      // Unlike `to`, a false guard here is a real destination, not a no-op — so the guard's inputs
      // must actually be present rather than defaulting. See MissingGuardContextError.
      assertBranchContext(transition.guard, ctx);
      const fires = evaluateGuard(transition.guard, ctx);
      return fires ? transition.when_true : transition.when_false;
    }
    case 'resume_prior': {
      if (!evaluateGuard(transition.guard, ctx)) return currentState;
      // A bare state string has no history to validate, so fail closed. The resume target comes
      // only from SessionSnapshot.interrupted_from, recorded by the owning session state.
      if (typeof current === 'string') return currentState;
      const recorded = current.interrupted_from;
      if (
        recorded === null ||
        recorded === 'INTERRUPTED' ||
        (ctx.prior_state !== undefined && ctx.prior_state !== recorded)
      ) {
        throw new InvalidResumeStateError(recorded, ctx.prior_state);
      }
      return recorded;
    }
    default: {
      const _exhaustive: never = transition;
      return _exhaustive;
    }
  }
}

// ---------------------------------------------------------------------------
// Invariant checker — INV2/INV3/INV4, decidable from a snapshot pair alone (contracts.ts's scope note).
// ---------------------------------------------------------------------------

export const checkSnapshotInvariants: SnapshotInvariantChecker = (snapshot, previous) => {
  const violations: InvariantViolation[] = [];

  // INV2 — steps spoken by index only, reference-not-generate: step_index must stay within
  // [0, steps_total] at all times (0 = no step presented yet).
  if (snapshot.step_index < 0 || snapshot.step_index > snapshot.steps_total) {
    violations.push({
      invariant: 'INV2',
      detail: `step_index ${snapshot.step_index} out of bounds for steps_total ${snapshot.steps_total}`,
    });
  }

  if (previous) {
    // INV3 is NOT checkable here and deliberately has no branch below: `SessionSnapshot` does not
    // carry the intent that caused a transition, so a recorded (WORKING -> STEP_DONE) pair is
    // indistinguishable from a legitimate one. The reducer's `explicit_done_intent` guard is the
    // only enforcement. If a snapshot ever gains `last_event`/`last_intent`, add the check here.

    // INV4 — session ends (SESSION_DONE -> IDLE_PRESENT) only on last-step-done + confirm.
    if (previous.state === 'SESSION_DONE' && snapshot.state === 'IDLE_PRESENT') {
      if (previous.step_index < previous.steps_total) {
        violations.push({
          invariant: 'INV4',
          detail: `session ended at step_index ${previous.step_index}/${previous.steps_total} — not last-step-done`,
        });
      }
    }
  }

  return violations;
};
