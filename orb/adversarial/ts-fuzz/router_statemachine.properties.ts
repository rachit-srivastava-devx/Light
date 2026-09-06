/**
 * H2-1 property-based tests (fast-check, MIT) for the pure control-plane logic: the router and the
 * session state machine. Track H2 (adversarial) -- black box, read-only import of product source,
 * no edits to apps/mobile/src.
 *
 * Run with:
 *   cd adversarial/ts-fuzz && npx --yes tsx router_statemachine.properties.ts
 *
 * fast-check is resolved from adversarial/ts-fuzz/node_modules (a real copy of the npm package,
 * fetched via `npx -p fast-check`, NOT hand-rolled -- see the H2 report for how it was obtained;
 * the root package.json/package-lock.json were never touched).
 */
import fc from 'fast-check';
import { route } from '../../apps/mobile/src/router/Router';
import type { IntentLabel, RouterAction } from '../../apps/mobile/src/router/contracts';
import { STUCK_REATOMIZE_THRESHOLD } from '../../apps/mobile/src/router/contracts';
import { dispatch, MissingGuardContextError, checkSnapshotInvariants } from '../../apps/mobile/src/session/StateMachine';
import type { SessionState, SessionEvent, SessionSnapshot } from '../../apps/mobile/src/session/contracts';

let passed = 0;
let failed = 0;
const failures: string[] = [];

function check(name: string, run: () => void): void {
  try {
    run();
    passed += 1;
    console.log(`PASS  ${name}`);
  } catch (err) {
    failed += 1;
    const msg = err instanceof Error ? err.message : String(err);
    failures.push(`${name}\n${msg}`);
    console.log(`FAIL  ${name}`);
    console.log(msg.split('\n').slice(0, 20).map((l) => '      ' + l).join('\n'));
  }
}

const SESSION_STATES: SessionState[] = [
  'IDLE_PRESENT', 'INTAKE', 'CLARIFY', 'STEP_PRESENT', 'WORKING', 'CHECK_IN', 'STEP_DONE',
  'INTERRUPTED', 'SESSION_DONE',
];
const INTENT_LABELS: IntentLabel[] = ['done', 'next', 'stuck', 'pause', 'question', 'chitchat'];
const ROUTER_ACTIONS: RouterAction[] = [
  'advance_step', 'pause_session', 'reanchor_templated', 'crew_reatomize', 'fast_voice', 'crew_atomize',
];
const SESSION_EVENTS: SessionEvent[] = [
  'tap', 'transcript', 'needs_clarification', 'clarify_answered', 'atomize_ready', 'spoken_complete',
  'policy_intervention', 'reply', 'intent_done', 'intent_next', 'step_advance', 'reatomize_ready',
  'user_pause', 'os_interrupt', 'os_resume', 'bed_faded', 'new_task',
];

const arbState = fc.constantFrom(...SESSION_STATES);
const arbIntent = fc.constantFrom(...INTENT_LABELS);
const arbEvent = fc.constantFrom(...SESSION_EVENTS);
// Deliberately adversarial: negative, huge, and non-integer stuck_count values -- the type says
// `number`, and nothing in Router.ts's signature stops a caller from passing something illegal.
const arbStuckCount = fc.oneof(
  fc.integer({ min: -1000, max: 1000 }),
  fc.double({ min: -1e6, max: 1e6, noNaN: false, noDefaultInfinity: false }),
);

// ---------------------------------------------------------------------------------------------
// Router.route -- totality: for ANY (state, intent, stuck_count) in the full type-level input
// space, route() must return one of the 6 known RouterAction labels and never throw.
// ---------------------------------------------------------------------------------------------
check('Router.route is total: never throws, always returns a valid RouterAction', () => {
  fc.assert(
    fc.property(arbState, arbIntent, arbStuckCount, (state, intent, stuck_count) => {
      const action = route(state, intent, stuck_count);
      return ROUTER_ACTIONS.includes(action);
    }),
    { numRuns: 20_000 },
  );
});

check('Router.route: state==INTAKE always wins (never shadowed by an intent row)', () => {
  fc.assert(
    fc.property(arbIntent, arbStuckCount, (intent, stuck_count) => {
      return route('INTAKE', intent, stuck_count) === 'crew_atomize';
    }),
    { numRuns: 5_000 },
  );
});

check('Router.route: stuck_count exactly at STUCK_REATOMIZE_THRESHOLD boundary is deterministic', () => {
  fc.assert(
    fc.property(arbState.filter((s) => s !== 'INTAKE'), (state) => {
      const below = route(state, 'stuck', STUCK_REATOMIZE_THRESHOLD - 1);
      const at = route(state, 'stuck', STUCK_REATOMIZE_THRESHOLD);
      const above = route(state, 'stuck', STUCK_REATOMIZE_THRESHOLD + 1);
      return below === 'reanchor_templated' && at === 'crew_reatomize' && above === 'crew_reatomize';
    }),
    { numRuns: 1_000 },
  );
});

// A NaN stuck_count is a real adversarial input (e.g., a corrupted counter, `0/0` upstream). The
// `>=` comparison against NaN is always false in JS, so this documents (rather than assumes) which
// branch NaN actually falls into -- silent-wrong-behaviour hunting, not just crash-hunting.
check('Router.route: NaN stuck_count does not throw (documents actual behaviour)', () => {
  fc.assert(
    fc.property(arbState, fc.constantFrom<IntentLabel>('stuck'), () => {
      const action = route('WORKING', 'stuck', NaN);
      if (action !== 'reanchor_templated') {
        throw new Error(
          `route(WORKING, stuck, NaN) = ${action}, expected 'reanchor_templated' (NaN >= threshold is false, ` +
          `so stuck_count=NaN silently behaves like stuck_count=0 -- a corrupted counter never escalates to crew_reatomize)`,
        );
      }
      return true;
    }),
    { numRuns: 1 },
  );
});

// ---------------------------------------------------------------------------------------------
// StateMachine.dispatch -- no illegal transition reachable. For ANY (state, event) and ANY guard
// context (including empty/adversarial), dispatch() must either (a) return a known SessionState,
// or (b) throw ONLY the documented MissingGuardContextError for the one branch guard that
// requires it. It must never return undefined, throw an unexpected error, or crash the process.
// ---------------------------------------------------------------------------------------------
const arbGuardContext = fc.record(
  {
    atomizer_output_valid: fc.boolean(),
    step_index: fc.integer({ min: -5, max: 20 }),
    steps_total: fc.integer({ min: -5, max: 20 }),
    policy_passed_veto_and_budget: fc.boolean(),
    intent: fc.oneof(fc.constantFrom('done', 'next', 'stuck', 'pause', 'question', 'chitchat', ''), fc.constant(null)),
    recovery_ms: fc.integer({ min: -1000, max: 100_000 }),
    recovery_budget_ms: fc.integer({ min: 0, max: 10_000 }),
    confirmed: fc.boolean(),
    slots_filled: fc.integer({ min: -5, max: 10 }),
    slots_required: fc.integer({ min: -5, max: 10 }),
    questions_asked: fc.integer({ min: -5, max: 10 }),
    question_cap: fc.integer({ min: -5, max: 10 }),
    prior_state: fc.constantFrom(...SESSION_STATES),
  },
  { requiredKeys: [] },
);

check('StateMachine.dispatch never returns an invalid SessionState, for any (state, event, ctx)', () => {
  let missingGuardCount = 0;
  let noopCount = 0;
  let transitionCount = 0;
  fc.assert(
    fc.property(arbState, arbEvent, arbGuardContext, (state, event, ctx) => {
      try {
        const next = dispatch(state, event, ctx as any);
        if (!SESSION_STATES.includes(next)) {
          throw new Error(`dispatch(${state}, ${event}) returned invalid state: ${String(next)}`);
        }
        if (next === state) noopCount += 1; else transitionCount += 1;
        return true;
      } catch (err) {
        if (err instanceof MissingGuardContextError) {
          missingGuardCount += 1;
          return true; // documented, typed failure mode -- acceptable
        }
        throw err;
      }
    }),
    { numRuns: 20_000 },
  );
  console.log(`      (transitions=${transitionCount}, no-ops=${noopCount}, typed MissingGuardContextError=${missingGuardCount})`);
});

// The FSM's ONLY branch edge is STEP_DONE.step_advance (has_more_steps). Every other (state,
// event) pair with a 'to' or 'resume_prior' shape must be safe under an EMPTY context (no
// MissingGuardContextError), because assertBranchContext only special-cases 'has_more_steps'.
check('StateMachine.dispatch: only STEP_DONE.step_advance can throw MissingGuardContextError', () => {
  fc.assert(
    fc.property(arbState, arbEvent, (state, event) => {
      try {
        dispatch(state, event, {});
        return true;
      } catch (err) {
        if (!(err instanceof MissingGuardContextError)) throw err;
        if (!(state === 'STEP_DONE' && event === 'step_advance')) {
          throw new Error(
            `dispatch(${state}, ${event}, {}) threw MissingGuardContextError, but the only edge ` +
            `documented to need branch context is STEP_DONE.step_advance. Either a new branch edge ` +
            `was added without updating assertBranchContext's allowlist, or this (state,event) pair ` +
            `unexpectedly routes through the branch path.`,
          );
        }
        return true;
      }
    }),
    { numRuns: 20_000 },
  );
});

// INV3/INV4 as reachability properties: no sequence of the FSM's own events can land on STEP_DONE
// without an explicit `intent_done`, and no sequence can reach IDLE_PRESENT from SESSION_DONE
// without `last_step_done_and_confirmed`. fast-check's commands/model runner explores sequences;
// here we do a bounded random-walk search (cheaper, still adversarial) for a counterexample.
check('No random event sequence reaches STEP_DONE via any event other than intent_done', () => {
  fc.assert(
    fc.property(fc.array(fc.tuple(arbEvent, arbGuardContext), { minLength: 1, maxLength: 40 }), (steps) => {
      let state: SessionState = 'WORKING';
      for (const [event, ctx] of steps) {
        let next: SessionState;
        try {
          next = dispatch(state, event, ctx as any);
        } catch (err) {
          if (err instanceof MissingGuardContextError) continue;
          throw err;
        }
        // Only a genuine (state -> STEP_DONE) TRANSITION counts; a no-op that merely stays at
        // STEP_DONE (an unmodelled event while already there, correctly ignored per "absent entry
        // means ignore") is not a reachability violation -- state !== 'STEP_DONE' guards that.
        if (next === 'STEP_DONE' && state !== 'STEP_DONE' && event !== 'intent_done') {
          throw new Error(
            `Reached STEP_DONE via event '${event}' (not intent_done) from state '${state}'. This is ` +
            `always via os_interrupt -> os_resume with an attacker/bug-controlled ctx.prior_state: ` +
            `resume_prior returns ctx.prior_state VERBATIM once 'bed_recovered' passes, with no check ` +
            `that it matches the FSM's own transition history. dispatch() is a pure function with no ` +
            `memory (by design, per StateMachine.ts's own comment), so THIS FILE cannot prove INV3/INV4 ` +
            `hold end-to-end -- that depends entirely on the caller (InterruptionHandler.ts / the ` +
            `session store, Track B territory, not audited here) always threading the FSM's own ` +
            `recorded interrupted_from back in and never a stale/racy value. Finding: the trust ` +
            `boundary for INV3/INV4 sits outside this pure function, with zero defense-in-depth inside it.`,
          );
        }
        state = next;
      }
      return true;
    }),
    { numRuns: 5_000 },
  );
});

check('checkSnapshotInvariants never throws on adversarial snapshot pairs', () => {
  const arbSnapshot = fc.record({
    session_id: fc.string(),
    state: arbState,
    step_index: fc.integer({ min: -100, max: 100 }),
    steps_total: fc.integer({ min: -100, max: 100 }),
    interrupted_from: fc.option(arbState, { nil: null }),
    bed_active: fc.boolean(),
  }) as fc.Arbitrary<SessionSnapshot>;
  fc.assert(
    fc.property(arbSnapshot, fc.option(arbSnapshot, { nil: null }), (snap, prev) => {
      checkSnapshotInvariants(snap, prev);
      return true;
    }),
    { numRuns: 5_000 },
  );
});

console.log(`\n=== router_statemachine.properties.ts: ${passed} passed, ${failed} failed (of ${passed + failed}) ===`);
if (failed > 0) process.exitCode = 1;
