/**
 * Build-mode plane contract — Register 0's parallel FSM (§4 of the contract).
 *
 * This file owns the vocabulary (states, events, guard IDs) for the build FSM.
 * It is file-disjoint from session/contracts.ts: no literal is shared between
 * the two unions, so a type error prevents cross-dispatch.
 */

/** §4.1 — the 8 build-mode states. */
export type BuildState =
  | 'BUILD_IDLE'
  | 'BUILD_INTAKE'
  | 'DECOMPOSE'
  | 'PROPOSE'
  | 'BUILD_CLARIFY'
  | 'PUSHBACK'
  | 'FREEZE_CONFIRM'
  | 'FROZEN';

/** §4.1 — every legal build event. */
export type BuildEvent =
  | 'enter_build'
  | 'goal_captured'
  | 'brief_drafted'
  | 'decompose_failed'
  | 'needs_slot'
  | 'slot_answered'
  | 'split_proposed'
  | 'user_pushback'
  | 'defend'
  | 'concede'
  | 'gate_ready'
  | 'gate_not_ready'
  | 'human_confirm'
  | 'human_reject'
  | 'exit_build';

/** §4.1 — guard IDs for the build transition table. */
export type BuildGuardId =
  | 'required_slots_filled'
  | 'gate_verdict_ready'
  | 'brief_schema_valid'
  | 'defence_carries_new_derivation';
