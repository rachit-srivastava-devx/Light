/**
 * Session plane contract — Register 0, the 9-state deterministic FSM.
 *
 * Source of truth: `docs/BUILD-DIGEST.md` §2 (FSM diagram + invariant list) and §4 (determinism:
 * state transitions contain zero LLM decisions). Nothing here has behaviour — `StateMachine.ts`
 * owns the transition table *data* and the reducer; this file fixes the vocabulary they must use
 * so parallel worktrees (Router, StepGate, ResponseEnvelope, Policy) compile against a frozen
 * surface (see `docs/adr/0001-parallel-build.md`).
 */

/** Wall-clock milliseconds since epoch. Integer (AGENTS.md invariant 6 — counts are never float). */
export type EpochMs = number;

/** §2 — "Session FSM (Register 0) — 9 states". Exactly these nine, no more. */
export type SessionState =
  | 'IDLE_PRESENT'
  | 'INTAKE'
  | 'CLARIFY'
  | 'STEP_PRESENT'
  | 'WORKING'
  | 'CHECK_IN'
  | 'STEP_DONE'
  | 'INTERRUPTED'
  | 'SESSION_DONE';

/**
 * §2 — one member per labelled edge in the FSM diagram:
 *   tap · transcript · atomize_ready · spoken_complete · policy emits intervention · reply ·
 *   intent==done · [more steps?] · os_interrupt · os_resume · bed fade.
 * `step_advance` is the diagram's `STEP_DONE --[more steps?]-->` branch; `bed_faded` is the
 * `SESSION_DONE --bed fade--> IDLE_PRESENT` edge.
 * `intent_next` is the explicit skip/next path: unlike `intent_done`, it cannot complete the
 * session and only presents another validated step when one exists.
 *
 * `needs_clarification`/`clarify_answered` resolve the CLARIFY spec gap (see `docs/adr/0003-*.md`):
 * blueprint doc 12 §4 states "Atomizing straight off a vague brain-dump produces vague steps. So
 * INTAKE → CLARIFY → ATOMIZE" — `ATOMIZE` there is the atomizer dispatch, not a session state, so
 * CLARIFY re-enters the same `atomize_ready` edge INTAKE uses once `ClarifyProtocol.ts`'s slots
 * (scope/first_context/blocker/time_box, max 2 questions) are filled.
 * `new_task` returns an active session to INTAKE when the user starts a different task before the
 * current one is complete; the current step is discarded and the new task is atomized normally.
 */
export type SessionEvent =
  | 'tap'
  | 'transcript'
  | 'needs_clarification'
  | 'clarify_answered'
  | 'atomize_ready'
  | 'spoken_complete'
  | 'policy_intervention'
  | 'reply'
  | 'intent_done'
  | 'intent_next'
  | 'step_advance'
  | 'reatomize_ready'
  | 'user_pause'
  | 'os_interrupt'
  | 'os_resume'
  | 'bed_faded'
  | 'new_task';

/**
 * §2 — the bracketed conditions on the diagram's edges, one id each. A guard is a pure predicate
 * over session data (§4: transitions are deterministic); `StateMachine.ts` owns the predicates.
 */
export type TransitionGuardId =
  /** `atomize_ready [steps schema-valid]` — the atomizer output passed validation (§2, INV2's precondition). */
  | 'steps_schema_valid'
  /** `spoken_complete [validated step exists, INV2]` — the spoken step came from the validated list by index. */
  | 'validated_step_exists'
  /** `policy emits intervention [passes veto+refractory+budget]` — see `cognitive/contracts.ts` (INV6). */
  | 'policy_admits'
  /** `intent==done [explicit, INV3]` — completion is never inferred from silence or a timeout. */
  | 'explicit_done_intent'
  /** `STEP_DONE --[more steps?]-->` — step_index < steps_total. */
  | 'has_more_steps'
  /** `os_resume [bed recovered ≤300ms]` — see `presence/contracts.ts` TRANSIENT_RECOVERY_BUDGET_MS. */
  | 'bed_recovered'
  /** `bed fade [last step done + confirm, INV4]` — session ends only on explicit intent + confirm. */
  | 'last_step_done_and_confirmed'
  /** `transcript [clarify slots incomplete]` — `ClarifyProtocol.ts` can't fill scope/first_context/blocker/time_box from the utterance alone. */
  | 'clarify_slots_incomplete'
  /** `clarify_answered [all required slots filled or 2-question cap hit]` — doc 12 §4 hard caps at 2 questions. */
  | 'clarify_slots_resolved';

/** One edge of the FSM. Three shapes because §2's diagram has three edge shapes. */
export type Transition =
  /** Plain edge, optionally guarded. `INTAKE --transcript--> INTAKE` (stay) is `to: 'INTAKE'`. */
  | { readonly kind: 'to'; readonly to: SessionState; readonly guard: TransitionGuardId | null }
  /** `STEP_DONE --[more steps?]--> STEP_PRESENT : SESSION_DONE` — the only two-target edge. */
  | {
      readonly kind: 'branch';
      readonly guard: TransitionGuardId;
      readonly when_true: SessionState;
      readonly when_false: SessionState;
    }
  /** `INTERRUPTED --os_resume--> prior state` — target is `SessionSnapshot.interrupted_from`. */
  | { readonly kind: 'resume_prior'; readonly guard: TransitionGuardId };

/**
 * The transition table type. Per-state map is partial because most (state, event) pairs are not
 * legal edges — an absent entry means "ignore the event", which is the FSM's only legal response
 * to an unmodelled input (§4: no inference, no fallback state guessing).
 *
 * `os_interrupt` is `any --os_interrupt--> INTERRUPTED` in §2, i.e. it appears in every state's map
 * except INTERRUPTED itself; the table is data, so `StateMachine.ts` writes those entries out
 * explicitly rather than special-casing them in the reducer.
 *
 * CLARIFY edges (resolved, `docs/adr/0003-register-enumeration.md`):
 *   INTAKE --transcript [clarify_slots_incomplete]--> CLARIFY
 *   CLARIFY --clarify_answered [clarify_slots_resolved]--> (stay; atomize dispatched, same as INTAKE)
 *   CLARIFY --atomize_ready [steps_schema_valid]--> STEP_PRESENT
 * `StateMachine.ts` writes these three entries explicitly; this file only fixed the vocabulary.
 */
export type TransitionTable = {
  readonly [S in SessionState]: { readonly [E in SessionEvent]?: Transition };
};

/**
 * INV1 (§2, AGENTS.md invariant 1): `bed_active == true` except in IDLE_PRESENT / SESSION_DONE or
 * the explicit pause contract (§5), which is only reachable from INTERRUPTED. These three states
 * are therefore the only ones where the bed may be silent.
 */
export type BedOptionalState = 'IDLE_PRESENT' | 'SESSION_DONE' | 'INTERRUPTED';
/** Every other state must carry a live bed — enforced at the type level in `SessionSnapshot`. */
export type BedRequiredState = Exclude<SessionState, BedOptionalState>;

interface SessionSnapshotBase {
  readonly session_id: string;
  /**
   * 1-based index of the step currently presented; `0` = no step presented yet.
   * 1-based per §2's envelope example (`step_card {index: 2, total: 6}` alongside
   * `progress {done: 1, total: 6}` — the second step in progress with one done).
   * Integer (AGENTS.md invariant 6).
   */
  readonly step_index: number;
  /** Integer, 1..12 once atomized (§2 `steps_total`); `0` before the atomizer has returned. */
  readonly steps_total: number;
  /** Target of `os_resume` (§2 `INTERRUPTED --os_resume--> prior state`); null outside INTERRUPTED. */
  readonly interrupted_from: SessionState | null;
}

/**
 * What a UI (and the response envelope's `session` block) reads. Union rather than a flat record so
 * INV1 is a compile error, not a test: a snapshot in WORKING with `bed_active: false` does not type.
 *
 * Residual runtime part of INV1: inside INTERRUPTED the bed may only be off for a *user-intent*
 * interruption (§5 pause contract). A *transient* interruption must keep or recover it within
 * 300ms — that distinction lives in `presence/contracts.ts` and cannot be expressed here.
 */
export type SessionSnapshot =
  | (SessionSnapshotBase & { readonly state: BedRequiredState; readonly bed_active: true })
  | (SessionSnapshotBase & { readonly state: BedOptionalState; readonly bed_active: boolean });

/** §2's invariant list, verbatim ids. Ownership noted so no two modules both claim to enforce one. */
export type InvariantId =
  /** bed_active == true except IDLE_PRESENT / SESSION_DONE / pause contract. Owner: presence + this file's type union. */
  | 'INV1'
  /** Steps spoken by index only, reference-not-generate. Owner: `session/StepGate.ts`. */
  | 'INV2'
  /** Completion only on explicit `done` intent. Owner: `session/StateMachine.ts`. */
  | 'INV3'
  /** Session ends only on last-step-done + confirm. Owner: `session/StateMachine.ts`. */
  | 'INV4'
  /** Per-session cost reservation, cannot exceed. Owner: `session/CostReservation.ts`. */
  | 'INV5'
  /** Refractory ≥90s + budget ≤6 interventions/session. Owner: `cognitive/Policy.ts`. */
  | 'INV6'
  /** Dysregulated>0.8 ⇒ interventions reduced to {Pause, Escalate}, shame-adjacent prosody excluded. Owner: `cognitive/Policy.ts` + `lld/ProsodyDirector.ts`. */
  | 'INV7';

export interface InvariantViolation {
  readonly invariant: InvariantId;
  /** Human-readable; carried into eval output (§6 CI gates), never parsed. */
  readonly detail: string;
}

/**
 * Signature only — the checker itself is `StateMachine.ts`'s to implement and the acceptance suite's
 * to exercise. Scoped to the invariants decidable from a snapshot alone (INV2–INV4); INV1 is a type
 * constraint above, INV5–INV7 are checked by their owning modules against state this shape lacks.
 */
export type SnapshotInvariantChecker = (
  snapshot: SessionSnapshot,
  previous: SessionSnapshot | null,
) => readonly InvariantViolation[];
