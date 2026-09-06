/**
 * Evidence cascade (`docs/BUILD-DIGEST.md` §2/§4).
 *
 * Tier0 and Tier1 return bounded evidence directly. Tier2 is represented only as a request for a
 * schema-locked classifier; this module never calls a model and never writes a belief value.
 */

import { cosineInt8 } from '../lld/ContextPack';
import {
  MAX_TIER2_EVIDENCE_LOGITS,
  type BeliefName,
  type Evidence,
  type EvidenceTier,
} from './contracts';

export interface EvidenceExample {
  readonly text: string;
  readonly embedding: readonly number[];
  readonly evidence: Evidence;
}

export interface EvidenceInput {
  readonly utterance: string;
  readonly embedding: readonly number[];
  readonly examples: readonly EvidenceExample[];
}

export type EvidenceDecision =
  | { readonly kind: 'evidence'; readonly evidence: Evidence; readonly source: EvidenceTier; readonly score: number }
  | {
      readonly kind: 'request_tier2';
      readonly reason: 'ambiguous_margin' | 'long_compositional';
      readonly candidates: readonly EvidenceExample[];
    }
  | { readonly kind: 'no_evidence'; readonly reason: 'empty_utterance' | 'invalid_embedding' | 'no_match' };

export const TIER1_MIN_SCORE = 0.72;
export const TIER1_AMBIGUITY_MARGIN = 0.15;
export const LONG_COMPOSITIONAL_WORDS = 18;

const RULES: readonly {
  readonly pattern: RegExp;
  readonly belief: BeliefName;
  readonly weight: number;
  readonly reliability: number;
}[] = [
  { pattern: /\b(stuck|blocked|can't|cannot)\b/i, belief: 'Execution', weight: -1.1, reliability: 0.9 },
  { pattern: /\b(overwhelmed|too much|panic|spiral)\b/i, belief: 'EmotionalLoad', weight: 1.2, reliability: 0.9 },
  { pattern: /\b(forgot|lost track|what was i doing)\b/i, belief: 'WorkingMemory', weight: -1, reliability: 0.85 },
  { pattern: /\b(done|finished|completed)\b/i, belief: 'Execution', weight: 1, reliability: 0.8 },
  { pattern: /\b(distracted|scrolling|wandered)\b/i, belief: 'Attention', weight: -0.9, reliability: 0.8 },
];

function words(text: string): readonly string[] {
  return text
    .trim()
    .toLowerCase()
    .split(/\s+/)
    .filter(Boolean);
}

function bounded(evidence: Evidence): Evidence {
  if (evidence.tier !== 'tier2') return evidence;
  const influence = evidence.weight * evidence.reliability;
  if (Math.abs(influence) <= MAX_TIER2_EVIDENCE_LOGITS) return evidence;
  const nextWeight = Math.sign(evidence.weight) * (MAX_TIER2_EVIDENCE_LOGITS / evidence.reliability);
  return { ...evidence, weight: nextWeight };
}

export function normalizeEvidence(evidence: Evidence): Evidence {
  if (!Number.isFinite(evidence.weight) || !Number.isFinite(evidence.reliability)) {
    throw new TypeError('Evidence weight and reliability must be finite numbers');
  }
  if (evidence.reliability < 0 || evidence.reliability > 1) {
    throw new RangeError('Evidence reliability must be in [0, 1]');
  }
  return bounded(evidence);
}

export function decideEvidence(input: EvidenceInput): EvidenceDecision {
  const text = input.utterance.trim();
  if (text.length === 0) return { kind: 'no_evidence', reason: 'empty_utterance' };
  if (input.embedding.length === 0 || input.embedding.some((n) => !Number.isFinite(n))) {
    return { kind: 'no_evidence', reason: 'invalid_embedding' };
  }

  for (const rule of RULES) {
    if (rule.pattern.test(text)) {
      return {
        kind: 'evidence',
        source: 'tier0',
        score: 1,
        evidence: normalizeEvidence({
          belief: rule.belief,
          weight: rule.weight,
          reliability: rule.reliability,
          tier: 'tier0',
        }),
      };
    }
  }

  const ranked = input.examples
    .map((example) => ({ example, score: cosineInt8(input.embedding, example.embedding) }))
    .filter((r) => r.score > 0)
    .sort((a, b) => b.score - a.score);

  const top = ranked[0];
  if (!top || top.score < TIER1_MIN_SCORE) {
    return { kind: 'no_evidence', reason: 'no_match' };
  }

  const second = ranked[1];
  const margin = top.score - (second?.score ?? 0);
  if (margin < TIER1_AMBIGUITY_MARGIN) {
    return {
      kind: 'request_tier2',
      reason: 'ambiguous_margin',
      candidates: ranked.slice(0, 2).map((r) => r.example),
    };
  }

  if (words(text).length >= LONG_COMPOSITIONAL_WORDS) {
    return {
      kind: 'request_tier2',
      reason: 'long_compositional',
      candidates: [top.example],
    };
  }

  return {
    kind: 'evidence',
    source: 'tier1',
    score: top.score,
    evidence: normalizeEvidence({ ...top.example.evidence, tier: 'tier1' }),
  };
}
