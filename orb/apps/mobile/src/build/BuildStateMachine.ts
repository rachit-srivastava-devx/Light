/**
 * Build-mode state machine — the 8-state deterministic FSM.
 *
 * Source of truth: `docs/SPEED-OF-THOUGHT-P0-CONTRACT.md` §4.1 (the transition
 * table). This file owns two things: the `BUILD_TRANSITION_TABLE` data and the
 * `buildDispatch` reducer that walks it.
 *
 * Determinism (§4): no wall-clock or randomness is read inside this file. Every
 * guard that needs external data takes it as an explicit parameter from the
 * caller, so replay is bit-identical.
 *
 * File-disjoint from session/{contracts,StateMachine}.ts — no shared literal
 * between BuildState and SessionState (U2-T4 asserts this).
 */

import type { Transition, TransitionTable } from './fsm-core';
import { makeDispatch } from './fsm-core';
import type { BuildEvent, BuildGuardId, BuildState } from './contracts';

// ---------------------------------------------------------------------------
// Transition table — the §4.1 FSM diagram as data.
// ---------------------------------------------------------------------------

type BuildTransition = Transition<BuildState, BuildGuardId>;
type BuildRow = { readonly [K in BuildEvent]?: BuildTransition };

const EXIT_EVERY_STATE: BuildRow = {
  exit_build: { kind: 'to', to: 'BUILD_IDLE', guard: null },
};

export const BUILD_TRANSITION_TABLE: TransitionTable<BuildState, BuildEvent, BuildGuardId> = {
  BUILD_IDLE: {
    enter_build: { kind: 'to', to: 'BUILD_INTAKE', guard: null },
    ...EXIT_EVERY_STATE,
  },

  BUILD_INTAKE: {
    goal_captured: { kind: 'to', to: 'DECOMPOSE', guard: null },
    ...EXIT_EVERY_STATE,
  },

  DECOMPOSE: {
    brief_drafted: { kind: 'to', to: 'PROPOSE', guard: 'brief_schema_valid' },
    decompose_failed: { kind: 'to', to: 'BUILD_CLARIFY', guard: null },
    ...EXIT_EVERY_STATE,
  },

  PROPOSE: {
    needs_slot: { kind: 'to', to: 'BUILD_CLARIFY', guard: null },
    user_pushback: { kind: 'to', to: 'PUSHBACK', guard: null },
    gate_ready: { kind: 'to', to: 'FREEZE_CONFIRM', guard: 'gate_verdict_ready' },
    gate_not_ready: { kind: 'to', to: 'BUILD_CLARIFY', guard: null },
    ...EXIT_EVERY_STATE,
  },

  BUILD_CLARIFY: {
    slot_answered: {
      kind: 'branch',
      guard: 'required_slots_filled',
      when_true: 'PROPOSE',
      when_false: 'BUILD_CLARIFY',
    },
    split_proposed: { kind: 'to', to: 'PROPOSE', guard: null },
    ...EXIT_EVERY_STATE,
  },

  PUSHBACK: {
    defend: { kind: 'to', to: 'PROPOSE', guard: 'defence_carries_new_derivation' },
    concede: { kind: 'to', to: 'DECOMPOSE', guard: null },
    ...EXIT_EVERY_STATE,
  },

  FREEZE_CONFIRM: {
    human_confirm: { kind: 'to', to: 'FROZEN', guard: null },
    human_reject: { kind: 'to', to: 'PROPOSE', guard: null },
    ...EXIT_EVERY_STATE,
  },

  FROZEN: {
    // FROZEN is terminal for P0 — no outgoing edges except exit_build.
    ...EXIT_EVERY_STATE,
  },
};

// ---------------------------------------------------------------------------
// Guard evaluation context — every parameter any build guard needs.
// ---------------------------------------------------------------------------

export interface BuildGuardContext {
  /** Brief passed `validateModuleBrief`. */
  readonly brief_valid?: boolean;
  /** All required build slots are non-null. */
  readonly required_slots_filled?: boolean;
  /** `lldReady(brief, refs).outcome === 'READY'`. */
  readonly gate_ready?: boolean;
  /** The defending turn carries a new derivation (not byte-equal to any prior). */
  readonly defence_new_derivation?: boolean;
}

function evaluateGuard(guard: BuildGuardId, ctx: BuildGuardContext): boolean {
  switch (guard) {
    case 'required_slots_filled':
      return ctx.required_slots_filled ?? false;
    case 'gate_verdict_ready':
      return ctx.gate_ready ?? false;
    case 'brief_schema_valid':
      return ctx.brief_valid ?? false;
    case 'defence_carries_new_derivation':
      return ctx.defence_new_derivation ?? false;
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
 * `buildDispatch(state, event, ctx) → nextState`. An absent (state, event)
 * pair is a no-op. A guard that evaluates false is also a no-op (for `to`
 * edges) or goes to the false branch (for `branch` edges).
 */
export const buildDispatch = makeDispatch<BuildState, BuildEvent, BuildGuardId, BuildGuardContext>(
  BUILD_TRANSITION_TABLE,
  evaluateGuard,
);
