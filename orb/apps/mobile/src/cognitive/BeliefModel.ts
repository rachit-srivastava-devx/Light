/**
 * The 9-belief vector and its update rule (`docs/BUILD-DIGEST.md` §2/§4).
 *
 * Beliefs are estimates, but their update is pure arithmetic in log-odds space, so the same
 * evidence stream replays bit-identically (§4). Nothing here reads a clock or calls a model: `now`
 * is injected, and an LLM may only supply *evidence* whose influence is capped
 * (`MAX_TIER2_EVIDENCE_LOGITS`), never a belief value directly.
 */

import {
  ATTENTION_CONFIDENCE_HALF_LIFE_MS,
  CONFIDENCE_GAIN_KAPPA,
  ENERGY_CONFIDENCE_HALF_LIFE_MS,
  MAX_TIER2_EVIDENCE_LOGITS,
  type BeliefDecayTable,
  type BeliefName,
  type BeliefState,
  type BeliefVector,
  type Evidence,
} from './contracts';
import type { EpochMs } from '../session/contracts';

/** Probability 0.5 == 0 logits. A belief with no evidence sits here at zero confidence. */
const PRIOR_VALUE = 0.5;

/**
 * Clamping bound for log-odds. Without it, repeated same-direction evidence drives `value` to
 * exactly 0 or 1, at which point `logit` returns ±Infinity and every subsequent update produces
 * NaN — the belief silently stops responding to evidence forever. 6 logits ≈ 0.9975, past any
 * policy threshold (§2's highest is 0.9), so clamping costs no expressiveness.
 */
const MAX_ABS_LOGIT = 6;

function logit(p: number): number {
  const clamped = Math.min(Math.max(p, 1e-6), 1 - 1e-6);
  return Math.log(clamped / (1 - clamped));
}

function sigmoid(x: number): number {
  return 1 / (1 + Math.exp(-x));
}

const BELIEF_NAMES: readonly BeliefName[] = [
  'Goal',
  'Context',
  'Attention',
  'Execution',
  'WorkingMemory',
  'Energy',
  'EmotionalLoad',
  'Trust',
  'NoveltyPull',
];

/**
 * §2 names two half-lives explicitly (Attention 90s, Energy 15min). The rest are this file's
 * judgment, interpolated between those two anchors by how fast the quantity actually moves:
 * moment-to-moment states (Execution, WorkingMemory, EmotionalLoad) track Attention; slow
 * dispositions (Goal, Context, Trust) are far slower. `T_rev` (reversion to prior) is 4x the
 * confidence half-life throughout — confidence should decay well before the estimate itself does,
 * so a stale belief reads as "unsure", not as "confidently wrong" (§2's calibration gate).
 */
const REVERSION_MULTIPLIER = 4;

function decayPair(confidenceHalfLifeMs: number) {
  return {
    confidence_half_life_ms: confidenceHalfLifeMs,
    reversion_half_life_ms: confidenceHalfLifeMs * REVERSION_MULTIPLIER,
  };
}

export const DEFAULT_DECAY_TABLE: BeliefDecayTable = {
  Attention: decayPair(ATTENTION_CONFIDENCE_HALF_LIFE_MS),
  Energy: decayPair(ENERGY_CONFIDENCE_HALF_LIFE_MS),
  Execution: decayPair(ATTENTION_CONFIDENCE_HALF_LIFE_MS),
  WorkingMemory: decayPair(ATTENTION_CONFIDENCE_HALF_LIFE_MS),
  EmotionalLoad: decayPair(ATTENTION_CONFIDENCE_HALF_LIFE_MS * 2),
  NoveltyPull: decayPair(ATTENTION_CONFIDENCE_HALF_LIFE_MS * 2),
  Goal: decayPair(ENERGY_CONFIDENCE_HALF_LIFE_MS),
  Context: decayPair(ENERGY_CONFIDENCE_HALF_LIFE_MS),
  Trust: decayPair(ENERGY_CONFIDENCE_HALF_LIFE_MS * 4),
};

export function initialBeliefs(): BeliefVector {
  const entry: BeliefState = { value: PRIOR_VALUE, confidence: 0, last_evidence_ts: null };
  return Object.fromEntries(BELIEF_NAMES.map((n) => [n, entry])) as BeliefVector;
}

/**
 * Applies one piece of evidence. Tier-2 (model-sourced) evidence has its total influence capped at
 * `MAX_TIER2_EVIDENCE_LOGITS` — §4's rule that an LLM can nudge a belief but never cross a policy
 * threshold on its own. Tier-0/1 evidence is deterministic and uncapped.
 */
export function applyEvidence(
  beliefs: BeliefVector,
  evidence: Evidence,
  now: EpochMs,
): BeliefVector {
  const current = beliefs[evidence.belief];
  // Evidence contracts declare reliability in [0, 1], but Tier-1 starts from cosine similarity,
  // whose raw range is [-1, 1]. Clamp at this pure boundary so a malformed upstream score cannot
  // create negative confidence or amplify evidence beyond its declared authority.
  const reliability = Number.isFinite(evidence.reliability)
    ? Math.min(1, Math.max(0, evidence.reliability))
    : 0;
  let delta = evidence.weight * reliability;

  if (evidence.tier === 'tier2') {
    delta = Math.sign(delta) * Math.min(Math.abs(delta), MAX_TIER2_EVIDENCE_LOGITS);
  }

  const nextLogit = Math.min(
    Math.max(logit(current.value) + delta, -MAX_ABS_LOGIT),
    MAX_ABS_LOGIT,
  );

  return {
    ...beliefs,
    [evidence.belief]: {
      value: sigmoid(nextLogit),
      confidence: Math.min(1, current.confidence + CONFIDENCE_GAIN_KAPPA * reliability),
      last_evidence_ts: now,
    },
  };
}

/**
 * Decays every belief toward the prior, and its confidence toward zero, for elapsed time with no
 * evidence (§2). Beliefs that never saw evidence are left untouched — they are already at the
 * prior with zero confidence, and decaying `null` would need a start time this function has no
 * honest way to pick.
 */
export function decay(
  beliefs: BeliefVector,
  now: EpochMs,
  table: BeliefDecayTable = DEFAULT_DECAY_TABLE,
): BeliefVector {
  const next: Record<string, BeliefState> = {};

  for (const name of BELIEF_NAMES) {
    const belief = beliefs[name];
    if (belief.last_evidence_ts === null) {
      next[name] = belief;
      continue;
    }
    const elapsed = now - belief.last_evidence_ts;
    if (elapsed <= 0) {
      next[name] = belief;
      continue;
    }
    const params = table[name];
    const confidenceFactor = Math.pow(2, -elapsed / params.confidence_half_life_ms);
    const reversion = 1 - Math.pow(2, -elapsed / params.reversion_half_life_ms);
    const currentLogit = logit(belief.value);
    const revertedLogit = currentLogit + (logit(PRIOR_VALUE) - currentLogit) * reversion;

    next[name] = {
      value: sigmoid(revertedLogit),
      confidence: belief.confidence * confidenceFactor,
      last_evidence_ts: belief.last_evidence_ts,
    };
  }

  return next as BeliefVector;
}
