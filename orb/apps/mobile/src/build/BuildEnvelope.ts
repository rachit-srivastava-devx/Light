/**
 * Status-echo envelope for build mode (Speed-of-Thought P0 contract §5).
 *
 * The echo reports ONLY what actually happened Orb-side: gate verdict, freeze stamped, or the named
 * reasons a brief was refused. It never says "building", "tested", or "PR #N" — nothing downstream
 * exists (§7). A fabricated status is a failing state, not a nicety (U5-T3 rejects any echo whose
 * speech carries downstream vocabulary: PR / merged / built / deployed / shipped / attested).
 *
 * Source of truth: `docs/SPEED-OF-THOUGHT-P0-CONTRACT.md` §5. This extends the frozen
 * `lld/ResponseEnvelope.ts` by the same superset pattern `ModedResponseEnvelope` uses — the envelope
 * is a frozen surface other worktrees compile against, so it is extended, never edited.
 */

import type { DepthScore } from './lld-v1';
import type { ResponseEnvelope } from '../lld/ResponseEnvelope';

/** A single governable check the lld-ready gate runs. Structurally mirrors LldReadyGate's `GateReason`. */
export interface GateReason {
  readonly check_id: string;
  readonly detail: string;
}

/** The pure gate verdict. Structurally mirrors LldReadyGate's `GateVerdict` (U3 shares no file with U5). */
export type GateVerdict =
  | { readonly outcome: 'READY'; readonly checked: number; readonly score: DepthScore }
  | { readonly outcome: 'NOT_READY'; readonly checked: number; readonly score: DepthScore;
      readonly reasons: readonly GateReason[] }
  | { readonly outcome: 'MEASURED_NOTHING'; readonly checked: 0 };

/**
 * What actually happened in the Orb. `handoff_status` is a const with exactly one legal value in P0:
 * nothing downstream of the frozen `lld.v1` artifact exists (§7), so the only honest claim is
 * "written, no consumer." When a consumer exists that is a schema change and a review, not a string edit.
 */
export type BuildBlock =
  | { readonly kind: 'gate_refused'; readonly node_id: string;
      readonly reasons: readonly GateReason[]; readonly score: DepthScore }
  | { readonly kind: 'awaiting_confirm'; readonly node_id: string; readonly score: DepthScore }
  | { readonly kind: 'frozen'; readonly node_id: string; readonly freeze_id: string;
      readonly content_hash: string; readonly version: number;
      readonly handoff_status: 'artifact_written_no_consumer' };

/** Superset of the frozen envelope, per §1.6 / §5. */
export interface BuildResponseEnvelope extends ResponseEnvelope {
  readonly build?: BuildBlock;
}
