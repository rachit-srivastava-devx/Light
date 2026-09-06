import { describe, expect, it } from 'vitest';

import { DEFAULT_DECAY_TABLE, applyEvidence, decay, initialBeliefs } from './BeliefModel';
import {
  LONG_COMPOSITIONAL_WORDS,
  TIER1_AMBIGUITY_MARGIN,
  decideEvidence,
  normalizeEvidence,
  type EvidenceExample,
} from './EvidenceTiers';
import { decide, evaluateGuards, refractoryWindowMs, type PolicyInput } from './Policy';
import {
  ATTENTION_CONFIDENCE_HALF_LIFE_MS,
  INTERVENTION_BUDGET_PER_SESSION,
  MAX_TIER2_EVIDENCE_LOGITS,
  REFRACTORY_FOCUSED_MS,
  REFRACTORY_MS,
  type BarrierRegister,
  type BeliefVector,
  type Evidence,
  type RegisterSet,
} from './contracts';
import type { SessionSnapshot } from '../session/contracts';

const NOW = 1_700_000_000_000;

describe('BeliefModel', () => {
  it('starts every belief at the prior with zero confidence', () => {
    const b = initialBeliefs();
    expect(b.Attention.value).toBe(0.5);
    expect(b.Attention.confidence).toBe(0);
    expect(b.Attention.last_evidence_ts).toBeNull();
  });

  it('moves a belief in the direction of the evidence and raises confidence', () => {
    const e: Evidence = { belief: 'Attention', weight: 1, reliability: 1, tier: 'tier0' };
    const b = applyEvidence(initialBeliefs(), e, NOW);
    expect(b.Attention.value).toBeGreaterThan(0.5);
    expect(b.Attention.confidence).toBeGreaterThan(0);
    expect(b.Attention.last_evidence_ts).toBe(NOW);
  });

  it('clamps out-of-range reliability before it can corrupt confidence', () => {
    const negative = applyEvidence(
      initialBeliefs(),
      { belief: 'Attention', weight: 1, reliability: -1, tier: 'tier1' },
      NOW,
    );
    expect(negative.Attention.value).toBe(0.5);
    expect(negative.Attention.confidence).toBe(0);

    const aboveOne = applyEvidence(
      initialBeliefs(),
      { belief: 'Attention', weight: 1, reliability: 2, tier: 'tier1' },
      NOW,
    );
    const exactlyOne = applyEvidence(
      initialBeliefs(),
      { belief: 'Attention', weight: 1, reliability: 1, tier: 'tier1' },
      NOW,
    );
    expect(aboveOne.Attention).toEqual(exactlyOne.Attention);
  });

  it('treats non-finite reliability as zero evidence', () => {
    for (const reliability of [Number.NaN, Number.POSITIVE_INFINITY]) {
      const result = applyEvidence(
        initialBeliefs(),
        { belief: 'Attention', weight: 1, reliability, tier: 'tier1' },
        NOW,
      );
      expect(result.Attention.value).toBe(0.5);
      expect(result.Attention.confidence).toBe(0);
    }
  });

  it('caps tier-2 (model) evidence so an LLM cannot cross a threshold alone', () => {
    // §4: a model may nudge a belief; it may never decide one.
    const huge: Evidence = { belief: 'Attention', weight: 50, reliability: 1, tier: 'tier2' };
    const capped = applyEvidence(initialBeliefs(), huge, NOW);
    const equivalent = applyEvidence(
      initialBeliefs(),
      { belief: 'Attention', weight: MAX_TIER2_EVIDENCE_LOGITS, reliability: 1, tier: 'tier0' },
      NOW,
    );
    expect(capped.Attention.value).toBeCloseTo(equivalent.Attention.value, 10);
  });

  it('does not cap deterministic tier-0 evidence', () => {
    const strong: Evidence = { belief: 'Attention', weight: 3, reliability: 1, tier: 'tier0' };
    const b = applyEvidence(initialBeliefs(), strong, NOW);
    expect(b.Attention.value).toBeGreaterThan(0.9);
  });

  it('never produces NaN under repeated saturating evidence', () => {
    // Without a logit clamp, value reaches exactly 1, logit returns Infinity, and every later
    // update is NaN — the belief silently stops responding forever.
    let b = initialBeliefs();
    const e: Evidence = { belief: 'Attention', weight: 5, reliability: 1, tier: 'tier0' };
    for (let i = 0; i < 200; i++) b = applyEvidence(b, e, NOW + i);
    expect(Number.isFinite(b.Attention.value)).toBe(true);
    expect(b.Attention.value).toBeLessThan(1);
    const after = applyEvidence(b, { ...e, weight: -5 }, NOW + 999);
    expect(after.Attention.value).toBeLessThan(b.Attention.value);
  });

  it('halves confidence after one half-life with no evidence', () => {
    const b = applyEvidence(
      initialBeliefs(),
      { belief: 'Attention', weight: 1, reliability: 1, tier: 'tier0' },
      NOW,
    );
    const decayed = decay(b, NOW + ATTENTION_CONFIDENCE_HALF_LIFE_MS, DEFAULT_DECAY_TABLE);
    expect(decayed.Attention.confidence).toBeCloseTo(b.Attention.confidence / 2, 6);
  });

  it('reverts a stale belief toward the prior', () => {
    const b = applyEvidence(
      initialBeliefs(),
      { belief: 'Attention', weight: 3, reliability: 1, tier: 'tier0' },
      NOW,
    );
    const decayed = decay(b, NOW + ATTENTION_CONFIDENCE_HALF_LIFE_MS * 20, DEFAULT_DECAY_TABLE);
    expect(decayed.Attention.value).toBeLessThan(b.Attention.value);
    expect(decayed.Attention.value).toBeGreaterThan(0.5);
  });

  it('leaves never-observed beliefs untouched by decay', () => {
    const b = initialBeliefs();
    const decayed = decay(b, NOW + 10_000_000, DEFAULT_DECAY_TABLE);
    expect(decayed.Trust).toEqual(b.Trust);
  });

  it('replays bit-identically for the same evidence stream', () => {
    const stream: Evidence[] = [
      { belief: 'Attention', weight: 1.2, reliability: 0.8, tier: 'tier0' },
      { belief: 'EmotionalLoad', weight: -0.4, reliability: 0.5, tier: 'tier1' },
      { belief: 'Attention', weight: 0.3, reliability: 0.9, tier: 'tier2' },
    ];
    const run = () => stream.reduce((b, e, i) => applyEvidence(b, e, NOW + i), initialBeliefs());
    expect(run()).toEqual(run());
  });
});

// ── Policy fixtures ──────────────────────────────────────────────────────────

const calmBarrier: BarrierRegister = {
  Blocked: 0,
  Uncertain: 0,
  Overwhelmed: 0,
  UnderStimulated: 0,
  Avoiding: 0,
  Waiting: 0,
  Fatigued: 0,
  Dysregulated: 0,
};

const neutralRegisters: RegisterSet = {
  goal: {
    NoGoal: 0,
    Formation: 0,
    Renegotiation: 0,
    Commitment: 1,
    Maintenance: 0,
    Completion: 0,
    Decay: 0,
  },
  attention: {
    Initiating: 0,
    Focused: 0.5,
    Exploring: 0,
    MindWandering: 0,
    Hyperfocus: 0,
    ExternalInterruption: 0,
    Recovering: 0,
  },
  barrier: calmBarrier,
};

const workingSession: SessionSnapshot = {
  session_id: 's1',
  state: 'WORKING',
  step_index: 1,
  steps_total: 3,
  interrupted_from: null,
  bed_active: true,
};

function input(overrides: Partial<PolicyInput> = {}): PolicyInput {
  return {
    beliefs: initialBeliefs(),
    registers: neutralRegisters,
    session: workingSession,
    history: [],
    now: NOW,
    session_elapsed_ms: 60_000,
    execution_stalled_ms: 0,
    step_just_completed: false,
    severity: 'normal',
    ...overrides,
  };
}

function withBarrier(over: Partial<BarrierRegister>): RegisterSet {
  return { ...neutralRegisters, barrier: { ...calmBarrier, ...over } };
}

describe('Policy — hard vetoes (ordered, first match wins)', () => {
  it('never pushes a dysregulated user, whatever else fired', () => {
    const d = decide(
      input({
        registers: { ...withBarrier({ Dysregulated: 0.9, Overwhelmed: 0.9 }) },
      }),
    );
    expect(d.reason).toBe('veto.dysregulated');
    expect(['Pause', 'Escalate']).toContain(d.intervention);
  });

  it('dysregulation outranks even critical severity', () => {
    const d = decide(
      input({ registers: withBarrier({ Dysregulated: 0.95 }), severity: 'critical' }),
    );
    expect(d.reason).toBe('veto.dysregulated');
  });

  it('yields the floor to session mechanics during INTAKE', () => {
    const d = decide(
      input({
        session: { ...workingSession, state: 'INTAKE' },
        registers: withBarrier({ Overwhelmed: 0.9 }),
      }),
    );
    expect(d.reason).toBe('veto.session_mechanics');
    expect(d.intervention).toBe('Observe');
  });

  it('stays quiet inside the refractory window', () => {
    const d = decide(
      input({
        history: [{ intervention: 'Suggest', at_ts: NOW - 1_000 }],
        registers: withBarrier({ Overwhelmed: 0.9 }),
      }),
    );
    expect(d.reason).toBe('veto.refractory');
  });

  it('lets critical severity through the refractory window', () => {
    const d = decide(
      input({
        history: [{ intervention: 'Suggest', at_ts: NOW - 1_000 }],
        registers: withBarrier({ Fatigued: 0.9 }),
        severity: 'critical',
      }),
    );
    expect(d.reason).not.toBe('veto.refractory');
  });

  it('stops once the session intervention budget is spent', () => {
    const history = Array.from({ length: INTERVENTION_BUDGET_PER_SESSION }, (_, i) => ({
      intervention: 'Suggest' as const,
      at_ts: NOW - REFRACTORY_MS * (i + 2),
    }));
    const d = decide(input({ history, registers: withBarrier({ Overwhelmed: 0.9 }) }));
    expect(d.reason).toBe('veto.budget_spent');
  });

  it('leaves a confidently focused user alone (do-no-harm)', () => {
    // Two observations: confidence gains kappa (0.4) per unit reliability, and the veto requires
    // > 0.6 — one event lands at 0.4 and correctly does NOT trigger it (asserted below).
    let beliefs: BeliefVector = initialBeliefs();
    for (let i = 0; i < 2; i++) {
      beliefs = applyEvidence(
        beliefs,
        { belief: 'Attention', weight: 2, reliability: 1, tier: 'tier0' },
        NOW,
      );
    }
    const registers: RegisterSet = {
      ...neutralRegisters,
      attention: { ...neutralRegisters.attention, Focused: 0.95 },
    };
    const d = decide(input({ beliefs, registers }));
    expect(d.reason).toBe('veto.do_no_harm');
    expect(d.intervention).toBe('Observe');
  });

  it('does not apply do-no-harm when focus is observed but not confidently', () => {
    const registers: RegisterSet = {
      ...neutralRegisters,
      attention: { ...neutralRegisters.attention, Focused: 0.95 },
      barrier: { ...calmBarrier, Fatigued: 0.9 },
    };
    // confidence stays 0 (no evidence applied), so the veto must not fire
    const d = decide(input({ registers }));
    expect(d.reason).not.toBe('veto.do_no_harm');
  });
});

describe('Policy — candidate guards (all evaluated, least intrusive wins)', () => {
  it('evaluates every guard rather than stopping at the first', () => {
    // LESSONS L3: transcribing this block as an if/elif chain would make the ordering decorative.
    const fired = evaluateGuards(
      input({
        registers: withBarrier({ Overwhelmed: 0.9, Fatigued: 0.9 }),
        step_just_completed: true,
      }),
    );
    const reasons = fired.map((f) => f.reason);
    expect(reasons).toContain('guard.overwhelmed');
    expect(reasons).toContain('guard.fatigued');
    expect(reasons).toContain('guard.step_completed');
  });

  it('picks the least intrusive intervention among those that fired', () => {
    // Celebrate(3) beats Suggest(5) and Pause(7) on the intrusiveness ladder.
    const d = decide(
      input({ registers: withBarrier({ Overwhelmed: 0.9, Fatigued: 0.9 }), step_just_completed: true }),
    );
    expect(d.intervention).toBe('Celebrate');
  });

  it('adds stimulus for an under-stimulated user rather than shrinking the step', () => {
    const d = decide(input({ registers: withBarrier({ UnderStimulated: 0.9 }) }));
    expect(d.intervention).toBe('Presence');
    expect(d.reason).toBe('guard.under_stimulated');
  });

  it('shrinks the step for an overwhelmed user (the opposite fix)', () => {
    const d = decide(input({ registers: withBarrier({ Overwhelmed: 0.9 }) }));
    expect(d.intervention).toBe('Suggest');
  });

  it('observes when nothing fires', () => {
    const d = decide(input());
    expect(d.reason).toBe('guard.none_fired');
    expect(d.intervention).toBe('Observe');
  });

  it('requires confidence before acting on a stall', () => {
    const d = decide(input({ execution_stalled_ms: 10 * 60_000 }));
    expect(d.reason).toBe('guard.none_fired');
  });

  it('acts on a stall once execution is confidently observed', () => {
    // Two observations: confidence gains kappa (0.4) per unit reliability, so one event lands at
    // 0.4 — below the guard's 0.5 bar. That is the guard working, not a fixture quirk.
    let beliefs = applyEvidence(
      initialBeliefs(),
      { belief: 'Execution', weight: 0.1, reliability: 1, tier: 'tier0' },
      NOW,
    );
    beliefs = applyEvidence(
      beliefs,
      { belief: 'Execution', weight: 0.1, reliability: 1, tier: 'tier0' },
      NOW,
    );
    const d = decide(input({ beliefs, execution_stalled_ms: 10 * 60_000 }));
    expect(d.reason).toBe('guard.execution_stalled');
  });
});

describe('Policy — commit semantics', () => {
  it('widens the refractory window while the user is focused', () => {
    expect(refractoryWindowMs(neutralRegisters)).toBe(REFRACTORY_MS);
    const focused: RegisterSet = {
      ...neutralRegisters,
      attention: { ...neutralRegisters.attention, Focused: 0.8 },
    };
    expect(refractoryWindowMs(focused)).toBe(REFRACTORY_FOCUSED_MS);
  });

  it('does not spend budget or start a refractory when merely observing', () => {
    // Otherwise a quiet policy locks itself out of ever speaking.
    const d = decide(input());
    expect(d.intervention).toBe('Observe');
    expect(d.budget_remaining).toBe(INTERVENTION_BUDGET_PER_SESSION);
    expect(d.refractory_until_ts).toBe(NOW);
  });

  it('spends budget and starts a refractory on a real intervention', () => {
    const d = decide(input({ registers: withBarrier({ Overwhelmed: 0.9 }) }));
    expect(d.budget_remaining).toBe(INTERVENTION_BUDGET_PER_SESSION - 1);
    expect(d.refractory_until_ts).toBe(NOW + REFRACTORY_MS);
  });

  it('is a pure function of its inputs', () => {
    const i = input({ registers: withBarrier({ Overwhelmed: 0.9 }) });
    expect(decide(i)).toEqual(decide(i));
  });

  it('does not assume history is sorted', () => {
    const history = [
      { intervention: 'Suggest' as const, at_ts: NOW - 500_000 },
      { intervention: 'Suggest' as const, at_ts: NOW - 1_000 },
      { intervention: 'Suggest' as const, at_ts: NOW - 300_000 },
    ];
    const d = decide(input({ history, registers: withBarrier({ Overwhelmed: 0.9 }) }));
    expect(d.reason).toBe('veto.refractory');
  });
});

describe('EvidenceTiers', () => {
  const example = (embedding: number[], belief: EvidenceExample['evidence']['belief'] = 'Attention'): EvidenceExample => ({
    text: 'attention example',
    embedding,
    evidence: { belief, weight: 0.5, reliability: 0.8, tier: 'tier1' },
  });

  it('emits tier0 evidence for obvious rule matches', () => {
    const d = decideEvidence({ utterance: "I'm stuck on this", embedding: [1, 0], examples: [] });
    expect(d.kind).toBe('evidence');
    if (d.kind === 'evidence') {
      expect(d.source).toBe('tier0');
      expect(d.evidence.belief).toBe('Execution');
    }
  });

  it('uses tier1 when cosine has a clear winner', () => {
    const d = decideEvidence({
      utterance: 'this resembles prior focus language',
      embedding: [1, 0],
      examples: [example([1, 0]), example([0, 1], 'Energy')],
    });
    expect(d.kind).toBe('evidence');
    if (d.kind === 'evidence') expect(d.source).toBe('tier1');
  });

  it('requests tier2 on a narrow top-two margin rather than guessing', () => {
    const d = decideEvidence({
      utterance: 'unclear but not rule matched',
      embedding: [1, 0],
      examples: [example([1, 0]), example([1, TIER1_AMBIGUITY_MARGIN / 2], 'Energy')],
    });
    expect(d.kind).toBe('request_tier2');
    if (d.kind === 'request_tier2') expect(d.reason).toBe('ambiguous_margin');
  });

  it('requests tier2 on long compositional text even with a winner', () => {
    const utterance = Array.from({ length: LONG_COMPOSITIONAL_WORDS }, (_, i) => `word${i}`).join(' ');
    const d = decideEvidence({ utterance, embedding: [1, 0], examples: [example([1, 0])] });
    expect(d.kind).toBe('request_tier2');
    if (d.kind === 'request_tier2') expect(d.reason).toBe('long_compositional');
  });

  it('fails closed on empty or corrupt input', () => {
    expect(decideEvidence({ utterance: ' ', embedding: [1], examples: [] })).toEqual({
      kind: 'no_evidence',
      reason: 'empty_utterance',
    });
    expect(decideEvidence({ utterance: 'hello', embedding: [Number.NaN], examples: [] })).toEqual({
      kind: 'no_evidence',
      reason: 'invalid_embedding',
    });
  });

  it('caps tier2 influence and rejects malformed reliability', () => {
    const capped = normalizeEvidence({ belief: 'Trust', weight: 50, reliability: 1, tier: 'tier2' });
    expect(Math.abs(capped.weight * capped.reliability)).toBe(MAX_TIER2_EVIDENCE_LOGITS);
    expect(() =>
      normalizeEvidence({ belief: 'Trust', weight: 1, reliability: 2, tier: 'tier2' }),
    ).toThrow(RangeError);
  });
});
