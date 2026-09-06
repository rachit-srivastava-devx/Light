/**
 * The pure status-echo formatter (Speed-of-Thought P0 contract §5, unit U5).
 *
 * Reports ONLY what actually happened Orb-side — gate verdict, freeze stamped, or the named reasons a
 * brief was refused. It never reports "building", "verifying", "merged", or "PR #N": nothing
 * downstream of the frozen `lld.v1` artifact exists (§7), and a fabricated status is a failing state,
 * not a nicety. `echoSpeech`'s spoken templates are deliberate so that U5-T3's denylist
 * (`PR | pull request | merged | merging | built | building | deployed | shipped | attested`) can
 * never be emitted.
 *
 * Pure: no clock, no randomness, no I/O. Deterministic given the same verdict and node.
 */

import type { BuildBlock, GateVerdict } from './BuildEnvelope';

/** The caller's freeze stamp when the gate returned READY and the human confirmed the freeze. */
export interface FreezeStamp {
  readonly freeze_id: string;
  readonly content_hash: string;
  readonly version: number;
}

/**
 * The output narrows to the honest Orb-side variant for the verdict you gave it: a refusal carries
 * the real reasons, a READY carries the freeze/awaiting shape. This keeps the frozen acceptance test
 * read `reasons`/`score` straight off the result while `BuildBlock` stays the §5 discriminated union.
 */
type StatusFor<V extends GateVerdict> = V extends { outcome: 'READY' }
  ? Extract<BuildBlock, { kind: 'frozen' | 'awaiting_confirm' }>
  : Extract<BuildBlock, { kind: 'gate_refused' }>;

function ratioPct(score: number): string {
  const pct = Math.round(score * 1000) / 10;
  return `${pct}%`;
}

/**
 * Map a pure gate verdict to the honest Orb-side status block.
 * - READY + a freeze stamp  → `frozen` (artifact written, no consumer).
 * - READY without a stamp   → `awaiting_confirm` (gate passed, human has not confirmed yet).
 * - NOT_READY anything      → `gate_refused` (the real reasons, never a generic apology).
 * - MEASURED_NOTHING        → `gate_refused` (a refusal, never a pass; zero checks measured).
 */
export function statusEcho<V extends GateVerdict>(
  verdict: V,
  nodeId: string,
  freeze?: FreezeStamp,
): StatusFor<V> {
  if (verdict.outcome === 'READY') {
    if (freeze) {
      return {
        kind: 'frozen',
        node_id: nodeId,
        freeze_id: freeze.freeze_id,
        content_hash: freeze.content_hash,
        version: freeze.version,
        handoff_status: 'artifact_written_no_consumer',
      } as StatusFor<V>;
    }
    return { kind: 'awaiting_confirm', node_id: nodeId, score: verdict.score } as StatusFor<V>;
  }
  return {
    kind: 'gate_refused',
    node_id: nodeId,
    reasons: verdict.outcome === 'NOT_READY' ? verdict.reasons : [],
    score: verdict.outcome === 'NOT_READY'
      ? verdict.score
      : { checks_total: 0, checks_passed: 0, ratio: 0, failed_check_ids: [] },
  } as StatusFor<V>;
}

/**
 * The spoken line for a status block. Generated from fixed templates that never name downstream work.
 */
export function echoSpeech(block: BuildBlock): string {
  switch (block.kind) {
    case 'gate_refused': {
      const ids = block.reasons.map((r) => r.check_id).join(', ') || 'none recorded';
      return `The brief for ${block.node_id} was refused. It passed ${ratioPct(block.score.ratio)} of the depth checks. The failing check${block.reasons.length === 1 ? '' : 's'}: ${ids}.`;
    }
    case 'awaiting_confirm': {
      return `The brief for ${block.node_id} clears every check. I am waiting on your confirmation to freeze it.`;
    }
    case 'frozen': {
      return `The brief for ${block.node_id} is frozen as ${block.freeze_id}, version ${block.version}. The artifact is written and sits on disk, but nothing consumes it yet.`;
    }
  }
}
