/**
 * H2-1 property-based tests (fast-check, MIT) for the belief/policy layer -- the two properties
 * the H2 brief calls out explicitly:
 *   1. "same evidence sequence => bit-identical belief trajectory" (pure-arithmetic determinism)
 *   2. "the do-no-harm veto must be UNREACHABLE to violate"
 * Track H2 (adversarial). Black box, read-only import, no edits to apps/mobile/src.
 *
 * Run with: cd adversarial/ts-fuzz && npx --yes tsx belief_policy.properties.ts
 */
import fc from 'fast-check';
import { applyEvidence, decay, initialBeliefs } from '../../apps/mobile/src/cognitive/BeliefModel';
import { decide } from '../../apps/mobile/src/cognitive/Policy';
import type { PolicyInput } from '../../apps/mobile/src/cognitive/Policy';
import {
  DYSREGULATED_INTERVENTION_SET,
  DYSREGULATED_VETO_THRESHOLD,
} from '../../apps/mobile/src/cognitive/contracts';
import type {
  BeliefName, BeliefVector, Evidence, EvidenceTier, RegisterSet, GoalState, AttentionState, BarrierFlag,
} from '../../apps/mobile/src/cognitive/contracts';
import type { SessionSnapshot } from '../../apps/mobile/src/session/contracts';

let passed = 0;
let failed = 0;

function check(name: string, run: () => void): void {
  try {
    run();
    passed += 1;
    console.log(`PASS  ${name}`);
  } catch (err) {
    failed += 1;
    const msg = err instanceof Error ? err.message : String(err);
    console.log(`FAIL  ${name}`);
    console.log(msg.split('\n').slice(0, 25).map((l) => '      ' + l).join('\n'));
  }
}

const BELIEF_NAMES: BeliefName[] = [
  'Goal', 'Context', 'Attention', 'Execution', 'WorkingMemory', 'Energy', 'EmotionalLoad', 'Trust', 'NoveltyPull',
];
const TIERS: EvidenceTier[] = ['tier0', 'tier1', 'tier2'];

const arbEvidence: fc.Arbitrary<Evidence> = fc.record({
  belief: fc.constantFrom(...BELIEF_NAMES),
  weight: fc.double({ min: -20, max: 20, noNaN: true }),
  reliability: fc.double({ min: -5, max: 5, noNaN: true }), // adversarial: reliability outside [0,1] too
  tier: fc.constantFrom(...TIERS),
});

// ---------------------------------------------------------------------------------------------
// Determinism: replaying the SAME (evidence, now) sequence twice from the SAME starting vector
// must produce bit-identical results (===, not just approximately equal) -- the exact claim the
// product makes ("pure arithmetic ... replays bit-identically").
// ---------------------------------------------------------------------------------------------
check('BeliefModel.applyEvidence: same evidence sequence => bit-identical trajectory (replay determinism)', () => {
  fc.assert(
    fc.property(
      fc.array(fc.tuple(arbEvidence, fc.integer({ min: 0, max: 10_000_000 })), { minLength: 1, maxLength: 50 }),
      (steps) => {
        let a = initialBeliefs();
        let b = initialBeliefs();
        for (const [evidence, now] of steps) {
          a = applyEvidence(a, evidence, now);
          b = applyEvidence(b, evidence, now);
        }
        for (const name of BELIEF_NAMES) {
          if (Object.is(a[name].value, -0) || Object.is(b[name].value, -0)) continue; // -0 vs 0 is not a real divergence
          if (a[name].value !== b[name].value || a[name].confidence !== b[name].confidence) {
            throw new Error(`Divergence on belief ${name}: run A = ${JSON.stringify(a[name])}, run B = ${JSON.stringify(b[name])}`);
          }
        }
        return true;
      },
    ),
    { numRuns: 3_000 },
  );
});

// Evidence.reliability is documented "0..1" (contracts.ts), but nothing in applyEvidence clamps or
// validates it at runtime -- and tier1 evidence is explicitly sourced from cosine similarity
// (contracts.ts: "Cosine over the ~150-utterance exemplar bank"), which naturally ranges [-1,1],
// not [0,1]. This generator intentionally includes negative reliability to test what happens if
// that documented-but-unenforced contract is ever violated upstream.
check('BeliefModel.applyEvidence: value and confidence always stay in [0,1], never NaN, for any evidence', () => {
  fc.assert(
    fc.property(fc.array(fc.tuple(arbEvidence, fc.integer({ min: 0, max: 10_000_000 })), { minLength: 1, maxLength: 100 }), (steps) => {
      let beliefs = initialBeliefs();
      for (const [evidence, now] of steps) {
        beliefs = applyEvidence(beliefs, evidence, now);
        for (const name of BELIEF_NAMES) {
          const { value, confidence } = beliefs[name];
          if (Number.isNaN(value) || Number.isNaN(confidence)) {
            throw new Error(`NaN belief after evidence ${JSON.stringify(evidence)}: ${name} = ${JSON.stringify(beliefs[name])}`);
          }
          if (value < 0 || value > 1) throw new Error(`value out of [0,1] for ${name}: ${value}`);
          if (confidence < 0 || confidence > 1) throw new Error(`confidence out of [0,1] for ${name}: ${confidence}`);
        }
      }
      return true;
    }),
    { numRuns: 3_000 },
  );
});

check('BeliefModel.decay: never produces NaN or out-of-[0,1] values, for any elapsed time', () => {
  fc.assert(
    fc.property(
      arbEvidence,
      fc.integer({ min: 0, max: 10_000_000 }),
      fc.integer({ min: 0, max: Number.MAX_SAFE_INTEGER }),
      (evidence, seedNow, elapsed) => {
        const seeded = applyEvidence(initialBeliefs(), evidence, seedNow);
        const decayed = decay(seeded, seedNow + elapsed);
        for (const name of BELIEF_NAMES) {
          const { value, confidence } = decayed[name];
          if (Number.isNaN(value) || Number.isNaN(confidence)) throw new Error(`NaN after decay(elapsed=${elapsed}) on ${name}`);
          if (value < 0 || value > 1) throw new Error(`decay produced out-of-range value ${value} for ${name}`);
          if (confidence < 0 || confidence > 1) throw new Error(`decay produced out-of-range confidence ${confidence} for ${name}`);
        }
        return true;
      },
    ),
    { numRuns: 3_000 },
  );
});

check('BeliefModel.decay: going backwards in time (elapsed<0) does not throw or corrupt state', () => {
  // Clocks are supposed to be injected/monotonic, but "now" arriving out of order (a race, a
  // clock adjustment) is a realistic adversarial input, not a contrived one.
  fc.assert(
    fc.property(arbEvidence, fc.integer({ min: 1000, max: 10_000_000 }), fc.integer({ min: 1, max: 999 }), (evidence, seedNow, backwards) => {
      const seeded = applyEvidence(initialBeliefs(), evidence, seedNow);
      const decayed = decay(seeded, seedNow - backwards);
      for (const name of BELIEF_NAMES) {
        if (Number.isNaN(decayed[name].value) || Number.isNaN(decayed[name].confidence)) {
          throw new Error(`decay with negative elapsed time produced NaN for ${name}`);
        }
      }
      return true;
    }),
    { numRuns: 1_000 },
  );
});

// ---------------------------------------------------------------------------------------------
// Policy.decide -- the do-no-harm veto and the dysregulated veto must be UNREACHABLE to violate.
// ---------------------------------------------------------------------------------------------
const GOAL_STATES: GoalState[] = ['NoGoal', 'Formation', 'Renegotiation', 'Commitment', 'Maintenance', 'Completion', 'Decay'];
const ATTENTION_STATES: AttentionState[] = ['Initiating', 'Focused', 'Exploring', 'MindWandering', 'Hyperfocus', 'ExternalInterruption', 'Recovering'];
const BARRIER_FLAGS: BarrierFlag[] = ['Blocked', 'Uncertain', 'Overwhelmed', 'UnderStimulated', 'Avoiding', 'Waiting', 'Fatigued', 'Dysregulated'];

const unit = fc.double({ min: 0, max: 1, noNaN: true });

function arbRegisterRecord<K extends string>(keys: readonly K[]): fc.Arbitrary<Record<K, number>> {
  const shape: Record<string, fc.Arbitrary<number>> = {};
  for (const k of keys) shape[k] = unit;
  return fc.record(shape) as fc.Arbitrary<Record<K, number>>;
}

const arbRegisters: fc.Arbitrary<RegisterSet> = fc.record({
  goal: arbRegisterRecord(GOAL_STATES),
  attention: arbRegisterRecord(ATTENTION_STATES),
  barrier: arbRegisterRecord(BARRIER_FLAGS),
}) as unknown as fc.Arbitrary<RegisterSet>;

const arbBeliefs: fc.Arbitrary<BeliefVector> = fc.record(
  Object.fromEntries(BELIEF_NAMES.map((n) => [n, fc.record({ value: unit, confidence: unit, last_evidence_ts: fc.constant(null) })])),
) as unknown as fc.Arbitrary<BeliefVector>;

// Sessions other than INTAKE/CLARIFY (those are hard-vetoed by veto.session_mechanics before the
// do-no-harm/dysregulated checks even run -- excluded here to target the two vetoes under test).
const arbLiveSessionState = fc.constantFrom('STEP_PRESENT', 'WORKING', 'CHECK_IN', 'STEP_DONE');

const arbSnapshot: fc.Arbitrary<SessionSnapshot> = fc.record({
  session_id: fc.string(),
  state: arbLiveSessionState,
  step_index: fc.integer({ min: 0, max: 12 }),
  steps_total: fc.integer({ min: 1, max: 12 }),
  interrupted_from: fc.constant(null),
  bed_active: fc.constant(true as const),
}) as unknown as fc.Arbitrary<SessionSnapshot>;

const arbPolicyInput: fc.Arbitrary<PolicyInput> = fc.record({
  beliefs: arbBeliefs,
  registers: arbRegisters,
  session: arbSnapshot,
  history: fc.array(fc.record({ intervention: fc.constantFrom(...DYSREGULATED_INTERVENTION_SET, 'Suggest', 'Observe' as const), at_ts: fc.integer({ min: 0, max: 10_000_000 }) }), { maxLength: 10 }),
  now: fc.integer({ min: 0, max: 10_000_000 }),
  session_elapsed_ms: fc.integer({ min: 0, max: 10_000_000 }),
  execution_stalled_ms: fc.integer({ min: 0, max: 10_000_000 }),
  step_just_completed: fc.boolean(),
  severity: fc.constantFrom('normal', 'critical'),
}) as unknown as fc.Arbitrary<PolicyInput>;

check('Policy.decide: Dysregulated>threshold ALWAYS yields {Pause,Escalate}, even at severity=critical, even with step_just_completed, even with budget/refractory exhausted', () => {
  fc.assert(
    fc.property(
      arbPolicyInput,
      fc.double({ min: DYSREGULATED_VETO_THRESHOLD + 1e-9, max: 1, noNaN: true }),
      (input, dysregulatedValue) => {
        const forced: PolicyInput = {
          ...input,
          registers: { ...input.registers, barrier: { ...input.registers.barrier, Dysregulated: dysregulatedValue } },
        };
        const decision = decide(forced);
        if (!(DYSREGULATED_INTERVENTION_SET as readonly string[]).includes(decision.intervention)) {
          throw new Error(
            `VETO BYPASSED: Dysregulated=${dysregulatedValue} (> threshold ${DYSREGULATED_VETO_THRESHOLD}) but decide() ` +
            `returned intervention='${decision.intervention}' (reason=${decision.reason}), not one of ${JSON.stringify(DYSREGULATED_INTERVENTION_SET)}. ` +
            `input=${JSON.stringify(forced)}`,
          );
        }
        return true;
      },
    ),
    { numRuns: 10_000 },
  );
});

check('Policy.decide: do-no-harm veto (confidently-focused, low-barrier) always yields Observe, regardless of severity', () => {
  fc.assert(
    fc.property(
      arbPolicyInput,
      fc.double({ min: 0.9 + 1e-6, max: 1, noNaN: true }),
      fc.double({ min: 0.6 + 1e-6, max: 1, noNaN: true }),
      fc.integer({ min: 0, max: 10_000_000 }),
      (input, focused, attnConfidence, now) => {
        // Construct the exact do-no-harm precondition: Focused>0.9, Attention.confidence>0.6, every
        // barrier<0.7, AND clear of every HIGHER-priority veto (dysregulated, session-mechanics,
        // refractory, budget -- history:[] and a fresh `now` isolate this from the refractory/budget
        // vetoes, which fire first and ALSO resolve to Observe; asserting reason too, not just
        // intervention, is what makes this a precise test of do-no-harm specifically).
        const lowBarrier = Object.fromEntries(BARRIER_FLAGS.map((f) => [f, 0.1])) as RegisterSet['barrier'];
        const forced: PolicyInput = {
          ...input,
          session: { ...input.session, state: 'WORKING' },
          history: [],
          now,
          registers: { ...input.registers, attention: { ...input.registers.attention, Focused: focused }, barrier: lowBarrier },
          beliefs: { ...input.beliefs, Attention: { ...input.beliefs.Attention, confidence: attnConfidence } },
        };
        const decision = decide(forced);
        if (decision.intervention !== 'Observe' || decision.reason !== 'veto.do_no_harm') {
          throw new Error(
            `DO-NO-HARM VETO BYPASSED: Focused=${focused}, Attention.confidence=${attnConfidence}, all barriers=0.1, history=[], ` +
            `but decide() returned intervention='${decision.intervention}' reason='${decision.reason}' (expected Observe/veto.do_no_harm). ` +
            `severity=${forced.severity}`,
          );
        }
        return true;
      },
    ),
    { numRuns: 10_000 },
  );
});

check('Policy.decide: budget_remaining in the returned decision is never negative', () => {
  fc.assert(
    fc.property(arbPolicyInput, (input) => {
      const decision = decide(input);
      if (decision.budget_remaining < 0) {
        throw new Error(`budget_remaining went negative: ${decision.budget_remaining}, history.length=${input.history.length}`);
      }
      return true;
    }),
    { numRuns: 5_000 },
  );
});

check('Policy.decide: refractory_until_ts is never before `now`', () => {
  fc.assert(
    fc.property(arbPolicyInput, (input) => {
      const decision = decide(input);
      if (decision.refractory_until_ts < input.now) {
        throw new Error(`refractory_until_ts=${decision.refractory_until_ts} < now=${input.now}`);
      }
      return true;
    }),
    { numRuns: 5_000 },
  );
});

check('Policy.decide never throws for any well-typed adversarial PolicyInput', () => {
  fc.assert(
    fc.property(arbPolicyInput, (input) => {
      decide(input);
      return true;
    }),
    { numRuns: 10_000 },
  );
});

console.log(`\n=== belief_policy.properties.ts: ${passed} passed, ${failed} failed (of ${passed + failed}) ===`);
if (failed > 0) process.exitCode = 1;
