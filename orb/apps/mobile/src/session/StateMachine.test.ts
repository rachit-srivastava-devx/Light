import { describe, expect, it } from 'vitest';
import {
  InvalidResumeStateError,
  MissingGuardContextError,
  SESSION_TRANSITION_TABLE,
  bed_recovered,
  checkSnapshotInvariants,
  clarify_slots_incomplete,
  clarify_slots_resolved,
  dispatch,
  explicit_done_intent,
  has_more_steps,
  last_step_done_and_confirmed,
  policy_admits,
  steps_schema_valid,
  validated_step_exists,
} from './StateMachine';
import type { SessionSnapshot, SessionState } from './contracts';

const ALL_STATES: readonly SessionState[] = [
  'IDLE_PRESENT',
  'INTAKE',
  'CLARIFY',
  'STEP_PRESENT',
  'WORKING',
  'CHECK_IN',
  'STEP_DONE',
  'INTERRUPTED',
  'SESSION_DONE',
];

describe('guard predicates', () => {
  it('steps_schema_valid mirrors the boolean it is given', () => {
    expect(steps_schema_valid(true)).toBe(true);
    expect(steps_schema_valid(false)).toBe(false);
  });

  it('validated_step_exists is true only within [1, steps_total]', () => {
    expect(validated_step_exists(0, 5)).toBe(false);
    expect(validated_step_exists(1, 5)).toBe(true);
    expect(validated_step_exists(5, 5)).toBe(true);
    expect(validated_step_exists(6, 5)).toBe(false);
  });

  it('policy_admits mirrors the verdict it is given', () => {
    expect(policy_admits(true)).toBe(true);
    expect(policy_admits(false)).toBe(false);
  });

  it('explicit_done_intent is true only for the literal "done" intent', () => {
    expect(explicit_done_intent('done')).toBe(true);
    expect(explicit_done_intent('next')).toBe(false);
    expect(explicit_done_intent(null)).toBe(false);
    // A silence/timeout event carries no intent string at all — must never read as done (INV3).
    expect(explicit_done_intent(undefined as unknown as string)).toBe(false);
  });

  it('has_more_steps is strict less-than', () => {
    expect(has_more_steps(1, 5)).toBe(true);
    expect(has_more_steps(5, 5)).toBe(false);
    expect(has_more_steps(6, 5)).toBe(false);
  });

  it('bed_recovered is within-budget-inclusive', () => {
    expect(bed_recovered(300, 300)).toBe(true);
    expect(bed_recovered(299, 300)).toBe(true);
    expect(bed_recovered(301, 300)).toBe(false);
  });

  it('last_step_done_and_confirmed requires both confirm and last-step', () => {
    expect(last_step_done_and_confirmed(5, 5, true)).toBe(true);
    expect(last_step_done_and_confirmed(4, 5, true)).toBe(false);
    expect(last_step_done_and_confirmed(5, 5, false)).toBe(false);
  });

  it('clarify_slots_incomplete/resolved are complementary boundary checks', () => {
    expect(clarify_slots_incomplete(2, 4)).toBe(true);
    expect(clarify_slots_incomplete(4, 4)).toBe(false);
    expect(clarify_slots_resolved(4, 4, 0, 2)).toBe(true);
    expect(clarify_slots_resolved(2, 4, 2, 2)).toBe(true); // question cap hit
    expect(clarify_slots_resolved(2, 4, 1, 2)).toBe(false);
  });
});

describe('dispatch — every legal transition fires', () => {
  it('IDLE_PRESENT --tap--> INTAKE', () => {
    expect(dispatch('IDLE_PRESENT', 'tap')).toBe('INTAKE');
  });

  it('INTAKE --transcript--> INTAKE (stay)', () => {
    expect(dispatch('INTAKE', 'transcript')).toBe('INTAKE');
  });

  it('INTAKE --needs_clarification [clarify_slots_incomplete]--> CLARIFY', () => {
    expect(
      dispatch('INTAKE', 'needs_clarification', { slots_filled: 1, slots_required: 4 }),
    ).toBe('CLARIFY');
  });

  it('INTAKE --needs_clarification--> no-op when slots are already complete', () => {
    expect(
      dispatch('INTAKE', 'needs_clarification', { slots_filled: 4, slots_required: 4 }),
    ).toBe('INTAKE');
  });

  it('INTAKE keeps its direct atomize_ready --> STEP_PRESENT edge (no clarification needed)', () => {
    expect(dispatch('INTAKE', 'atomize_ready', { atomizer_output_valid: true })).toBe(
      'STEP_PRESENT',
    );
  });

  it('INTAKE --atomize_ready--> no-op when the atomizer output failed validation', () => {
    expect(dispatch('INTAKE', 'atomize_ready', { atomizer_output_valid: false })).toBe('INTAKE');
  });

  it('CLARIFY --clarify_answered [clarify_slots_resolved]--> stays, dispatch happens outside FSM', () => {
    expect(
      dispatch('CLARIFY', 'clarify_answered', {
        slots_filled: 4,
        slots_required: 4,
        questions_asked: 1,
        question_cap: 2,
      }),
    ).toBe('CLARIFY');
  });

  it('CLARIFY --clarify_answered--> no-op while slots remain unresolved and cap not hit', () => {
    expect(
      dispatch('CLARIFY', 'clarify_answered', {
        slots_filled: 1,
        slots_required: 4,
        questions_asked: 0,
        question_cap: 2,
      }),
    ).toBe('CLARIFY');
  });

  it('CLARIFY --atomize_ready--> STEP_PRESENT', () => {
    expect(dispatch('CLARIFY', 'atomize_ready', { atomizer_output_valid: true })).toBe(
      'STEP_PRESENT',
    );
  });

  it('STEP_PRESENT --spoken_complete [validated step exists]--> WORKING', () => {
    expect(
      dispatch('STEP_PRESENT', 'spoken_complete', { step_index: 1, steps_total: 5 }),
    ).toBe('WORKING');
  });

  it('STEP_PRESENT --spoken_complete--> no-op when the step index is out of bounds', () => {
    expect(
      dispatch('STEP_PRESENT', 'spoken_complete', { step_index: 0, steps_total: 5 }),
    ).toBe('STEP_PRESENT');
  });

  it('WORKING --policy_intervention [policy_admits]--> CHECK_IN', () => {
    expect(
      dispatch('WORKING', 'policy_intervention', { policy_passed_veto_and_budget: true }),
    ).toBe('CHECK_IN');
  });

  it('WORKING --policy_intervention--> no-op when the policy vetoed', () => {
    expect(
      dispatch('WORKING', 'policy_intervention', { policy_passed_veto_and_budget: false }),
    ).toBe('WORKING');
  });

  it('CHECK_IN --reply--> WORKING', () => {
    expect(dispatch('CHECK_IN', 'reply')).toBe('WORKING');
  });

  it('WORKING --intent_done [explicit_done_intent]--> STEP_DONE', () => {
    expect(dispatch('WORKING', 'intent_done', { intent: 'done' })).toBe('STEP_DONE');
  });

  it('WORKING --intent_next [has_more_steps]--> STEP_PRESENT without entering STEP_DONE', () => {
    expect(dispatch('WORKING', 'intent_next', { step_index: 1, steps_total: 2 })).toBe('STEP_PRESENT');
  });

  it('WORKING --intent_next--> no-op when there is no next step', () => {
    expect(dispatch('WORKING', 'intent_next', { step_index: 2, steps_total: 2 })).toBe('WORKING');
  });

  it('STEP_DONE --step_advance--> STEP_PRESENT when more steps remain', () => {
    expect(
      dispatch('STEP_DONE', 'step_advance', { step_index: 2, steps_total: 5 }),
    ).toBe('STEP_PRESENT');
  });

  it('STEP_DONE --step_advance--> SESSION_DONE when no more steps remain', () => {
    expect(
      dispatch('STEP_DONE', 'step_advance', { step_index: 5, steps_total: 5 }),
    ).toBe('SESSION_DONE');
  });

  // Regression (Opus review, 2026-08-04): the branch edge used to default its guard inputs to 0,
  // so `0 < 0 == false` sent an under-populated dispatch straight to SESSION_DONE — a caller that
  // forgot to fill GuardContext would silently end the session and let the bed go quiet
  // (INV4 + INV1). A branch edge now demands its inputs instead of defaulting them.
  it('STEP_DONE --step_advance--> throws rather than ending the session on an empty context', () => {
    expect(() => dispatch('STEP_DONE', 'step_advance', {})).toThrow(MissingGuardContextError);
    expect(() => dispatch('STEP_DONE', 'step_advance', { step_index: 2 })).toThrow(
      MissingGuardContextError,
    );
    expect(() => dispatch('STEP_DONE', 'step_advance', { steps_total: 5 })).toThrow(
      MissingGuardContextError,
    );
    expect(() => dispatch('STEP_DONE', 'step_advance', { step_index: 2.5, steps_total: 5 })).toThrow(
      MissingGuardContextError,
    );
  });

  it('any --os_interrupt--> INTERRUPTED, except INTERRUPTED itself', () => {
    for (const state of ALL_STATES) {
      if (state === 'INTERRUPTED') continue;
      expect(dispatch(state, 'os_interrupt')).toBe('INTERRUPTED');
    }
  });

  it('any --user_pause--> INTERRUPTED, except INTERRUPTED itself', () => {
    for (const state of ALL_STATES) {
      if (state === 'INTERRUPTED') continue;
      expect(dispatch(state, 'user_pause')).toBe('INTERRUPTED');
    }
  });

  it('INTERRUPTED has no os_interrupt entry (no-op, stays INTERRUPTED)', () => {
    expect(dispatch('INTERRUPTED', 'os_interrupt')).toBe('INTERRUPTED');
  });

  it('INTERRUPTED --os_resume [bed recovered]--> prior state', () => {
    const interrupted: SessionSnapshot = {
      session_id: 's1',
      state: 'INTERRUPTED',
      bed_active: true,
      step_index: 2,
      steps_total: 5,
      interrupted_from: 'WORKING',
    };
    expect(
      dispatch(interrupted, 'os_resume', {
        recovery_ms: 200,
        recovery_budget_ms: 300,
        prior_state: 'WORKING',
      }),
    ).toBe('WORKING');
  });

  it('INTERRUPTED --os_resume--> no-op (stays INTERRUPTED) when bed did not recover in time', () => {
    const interrupted: SessionSnapshot = {
      session_id: 's1',
      state: 'INTERRUPTED',
      bed_active: false,
      step_index: 2,
      steps_total: 5,
      interrupted_from: 'WORKING',
    };
    expect(
      dispatch(interrupted, 'os_resume', {
        recovery_ms: 400,
        recovery_budget_ms: 300,
        prior_state: 'WORKING',
      }),
    ).toBe('INTERRUPTED');
  });

  it('rejects a caller resume target that disagrees with the recorded interruption history', () => {
    const interrupted: SessionSnapshot = {
      session_id: 's1',
      state: 'INTERRUPTED',
      bed_active: true,
      step_index: 2,
      steps_total: 5,
      interrupted_from: 'WORKING',
    };
    expect(() =>
      dispatch(interrupted, 'os_resume', {
        recovery_ms: 200,
        recovery_budget_ms: 300,
        prior_state: 'STEP_DONE',
      }),
    ).toThrow(InvalidResumeStateError);
  });

  it('fails closed when resume is attempted without an interruption snapshot', () => {
    expect(
      dispatch('INTERRUPTED', 'os_resume', {
        recovery_ms: 200,
        recovery_budget_ms: 300,
        prior_state: 'STEP_DONE',
      }),
    ).toBe('INTERRUPTED');
  });

  it('SESSION_DONE --bed_faded [last_step_done_and_confirmed]--> IDLE_PRESENT', () => {
    expect(
      dispatch('SESSION_DONE', 'bed_faded', { step_index: 5, steps_total: 5, confirmed: true }),
    ).toBe('IDLE_PRESENT');
  });

  it('SESSION_DONE --bed_faded--> no-op without confirm', () => {
    expect(
      dispatch('SESSION_DONE', 'bed_faded', { step_index: 5, steps_total: 5, confirmed: false }),
    ).toBe('SESSION_DONE');
  });
});

describe('dispatch — illegal (state, event) pairs are no-ops', () => {
  it('an unmodelled event on a state with no matching entry does not change state', () => {
    expect(dispatch('IDLE_PRESENT', 'transcript')).toBe('IDLE_PRESENT');
    expect(dispatch('WORKING', 'tap')).toBe('WORKING');
    expect(dispatch('CHECK_IN', 'intent_done')).toBe('CHECK_IN');
    expect(dispatch('SESSION_DONE', 'tap')).toBe('SESSION_DONE');
  });

  it('every state has exactly the entries the FSM diagram gives it (table shape check)', () => {
    expect(Object.keys(SESSION_TRANSITION_TABLE.IDLE_PRESENT).sort()).toEqual([
      'os_interrupt',
      'tap',
      'user_pause',
    ]);
    expect(Object.keys(SESSION_TRANSITION_TABLE.INTERRUPTED).sort()).toEqual(['os_resume']);
    expect(Object.keys(SESSION_TRANSITION_TABLE.SESSION_DONE).sort()).toEqual([
      'bed_faded',
      'new_task',
      'os_interrupt',
      'user_pause',
    ]);
  });
});

describe('INV3 — completion only on explicit done intent, never inferred', () => {
  it('a silence/timeout-shaped event (no intent_done) must not complete a step from WORKING', () => {
    // Simulate a timeout by dispatching an event that isn't in WORKING's map at all.
    expect(dispatch('WORKING', 'transcript')).toBe('WORKING');
    expect(dispatch('WORKING', 'reply')).toBe('WORKING');
  });

  it('intent_done with a non-"done" intent value does not complete the step', () => {
    expect(dispatch('WORKING', 'intent_done', { intent: 'next' })).toBe('WORKING');
    expect(dispatch('WORKING', 'intent_done', { intent: null })).toBe('WORKING');
    expect(dispatch('WORKING', 'intent_done', {})).toBe('WORKING');
  });

  it('intent_next never reaches STEP_DONE', () => {
    expect(dispatch('WORKING', 'intent_next', { step_index: 1, steps_total: 2 })).toBe('STEP_PRESENT');
  });

  it('STEP_DONE is reachable only via the intent_done edge gated on explicit_done_intent', () => {
    const events = Object.keys(SESSION_TRANSITION_TABLE.WORKING);
    const intoStepDone = events.filter((e) => {
      const t = SESSION_TRANSITION_TABLE.WORKING[e as keyof typeof SESSION_TRANSITION_TABLE.WORKING];
      return t && t.kind === 'to' && t.to === 'STEP_DONE';
    });
    expect(intoStepDone).toEqual(['intent_done']);
  });
});

describe('INV4 — session ends only on last-step-done + confirm', () => {
  it('bed_faded does not fire without last_step_done_and_confirmed', () => {
    expect(
      dispatch('SESSION_DONE', 'bed_faded', { step_index: 3, steps_total: 5, confirmed: true }),
    ).toBe('SESSION_DONE');
  });

  it('checkSnapshotInvariants flags an INV4 violation if a replayed pair ends early', () => {
    const previous: SessionSnapshot = {
      session_id: 's1',
      state: 'SESSION_DONE',
      bed_active: false,
      step_index: 3,
      steps_total: 5,
      interrupted_from: null,
    };
    const snapshot: SessionSnapshot = {
      session_id: 's1',
      state: 'IDLE_PRESENT',
      bed_active: false,
      step_index: 3,
      steps_total: 5,
      interrupted_from: null,
    };
    const violations = checkSnapshotInvariants(snapshot, previous);
    expect(violations).toContainEqual(
      expect.objectContaining({ invariant: 'INV4' }),
    );
  });

  it('checkSnapshotInvariants does not flag INV4 when the session ended at the last step', () => {
    const previous: SessionSnapshot = {
      session_id: 's1',
      state: 'SESSION_DONE',
      bed_active: false,
      step_index: 5,
      steps_total: 5,
      interrupted_from: null,
    };
    const snapshot: SessionSnapshot = {
      session_id: 's1',
      state: 'IDLE_PRESENT',
      bed_active: false,
      step_index: 5,
      steps_total: 5,
      interrupted_from: null,
    };
    expect(checkSnapshotInvariants(snapshot, previous)).toEqual([]);
  });
});

describe('checkSnapshotInvariants — INV2 step_index bounds', () => {
  it('flags an out-of-bounds step_index', () => {
    const snapshot: SessionSnapshot = {
      session_id: 's1',
      state: 'WORKING',
      bed_active: true,
      step_index: 9,
      steps_total: 5,
      interrupted_from: null,
    };
    const violations = checkSnapshotInvariants(snapshot, null);
    expect(violations).toContainEqual(expect.objectContaining({ invariant: 'INV2' }));
  });

  it('does not flag a step_index within bounds', () => {
    const snapshot: SessionSnapshot = {
      session_id: 's1',
      state: 'WORKING',
      bed_active: true,
      step_index: 2,
      steps_total: 5,
      interrupted_from: null,
    };
    expect(checkSnapshotInvariants(snapshot, null)).toEqual([]);
  });
});
