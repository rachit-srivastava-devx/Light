/**
 * Generic FSM core — extracted from session/StateMachine.ts's reducer shape.
 *
 * Three edge shapes (`to` / `branch` / `resume_prior`) with guard evaluation,
 * parameterised over state, event, and guard ID types. Pure, no I/O, no clock,
 * no randomness.
 *
 * Per the contract (§4), this is a build-new extraction: the session FSM is
 * NOT edited. The session FSM could later migrate onto this core (C2 extract)
 * once its uncommitted edits settle.
 */

/** The three edge shapes shared by both session and build FSMs. */
export type Transition<S extends string, G extends string> =
  | { readonly kind: 'to'; readonly to: S; readonly guard: G | null }
  | { readonly kind: 'branch'; readonly guard: G; readonly when_true: S; readonly when_false: S }
  | { readonly kind: 'resume_prior'; readonly guard: G };

/** Per-state map of event → transition. Partial: absent entries mean "ignore the event". */
export type TransitionTable<S extends string, E extends string, G extends string = string> = {
  readonly [Key in S]?: { readonly [K in E]?: Transition<S, G> };
};

/**
 * Evaluate a guard predicate. Each concrete FSM supplies its own mapping from
 * guard ID to a pure predicate over its guard context.
 */
export type GuardEvaluator<G extends string, Ctx> = (guard: G, ctx: Ctx) => boolean;

/**
 * `dispatch(state, event, ctx) → nextState`. The reducer for a transition table.
 *
 * An absent (state, event) pair is a no-op — the FSM stays put. A `to` edge
 * whose guard evaluates false is also a no-op. A `branch` edge commits to one
 * of two targets regardless of guard outcome (hence `MissingGuardContextError`
 * discipline). A `resume_prior` edge resumes to the recorded prior state from
 * a snapshot (caller validates the snapshot).
 */
export function makeDispatch<S extends string, E extends string, G extends string, Ctx>(
  table: TransitionTable<S, E, G>,
  evaluateGuard: GuardEvaluator<G, Ctx>,
): (current: S, event: E, ctx?: Ctx) => S {
  return (current, event, ctx = {} as Ctx): S => {
    const stateMap = table[current];
    if (!stateMap) return current;

    const transition = stateMap[event];
    if (!transition) return current;

    switch (transition.kind) {
      case 'to': {
        if (transition.guard !== null && !evaluateGuard(transition.guard, ctx)) return current;
        return transition.to;
      }
      case 'branch': {
        const fires = evaluateGuard(transition.guard, ctx);
        return fires ? transition.when_true : transition.when_false;
      }
      case 'resume_prior': {
        if (!evaluateGuard(transition.guard, ctx)) return current;
        return current;
      }
      default: {
        const _exhaustive: never = transition;
        return _exhaustive;
      }
    }
  };
}
