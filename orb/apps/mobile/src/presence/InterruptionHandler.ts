/**
 * Classifies OS audio events transient vs user-intent and drives recovery, per
 * `docs/BUILD-DIGEST.md` §5: pure decision logic (event in, action out) — no actual OS API calls,
 * those belong to a native bridge this task doesn't build.
 *
 * §5 quotes verbatim:
 *   "Transient interruptions (call, Siri, audio-stack reset): resume context, ramp bed ≤300ms +
 *   soft swell; audio-stack death may exceed 300ms → spoken cover line."
 *   "triggered by backgrounding, screen lock, Bluetooth/headphone disconnect, or another app
 *   playing media (all 'user-intent')" → full pause contract, explicit resume only.
 */

import { PAUSE_CONTRACT_SEQUENCE, TRANSIENT_RECOVERY_BUDGET_MS } from './contracts';
import type { InterruptionClassifier, InterruptionEvent, PauseContractStep } from './contracts';

/**
 * §5 — only `audio_stack_reset` is called out as possibly exceeding the 300ms budget ("audio-stack
 * death may exceed 300ms → spoken cover line"); `incoming_call`/`siri` get the plain ramp+swell
 * recovery within budget. `cover_required` is therefore keyed on cause, not on `kind` alone — the
 * contract's own comment ("True only for transient events") bounds it to this branch, not that
 * every transient cause needs one.
 */
export const classifyInterruption: InterruptionClassifier = (event: InterruptionEvent) => {
  if (event.kind === 'user_intent') {
    return { kind: 'user_intent', resume: 'explicit_user_tap', cover_required: false };
  }
  return {
    kind: 'transient',
    resume: 'automatic_with_cover',
    cover_required: event.cause === 'audio_stack_reset',
  };
};

export type RecoveryAction = 'ramp_and_swell' | 'ramp_with_cover';

export interface RecoveryPlan {
  readonly action: RecoveryAction;
  readonly budget_ms: typeof TRANSIENT_RECOVERY_BUDGET_MS;
}

/** §5 — the transient recovery: ramp the bed back within `TRANSIENT_RECOVERY_BUDGET_MS`. */
export function planRecovery(event: Extract<InterruptionEvent, { kind: 'transient' }>): RecoveryPlan {
  return {
    action: event.cause === 'audio_stack_reset' ? 'ramp_with_cover' : 'ramp_and_swell',
    budget_ms: TRANSIENT_RECOVERY_BUDGET_MS,
  };
}

/**
 * §5 — the five pause-contract steps, in the fixed order the contract requires (mic released
 * before the UI claims it isn't listening). Re-exported by name here rather than re-declared so
 * `InterruptionHandler.ts` is the single call site session code drives off of.
 */
export function pauseContractSteps(): readonly PauseContractStep[] {
  return PAUSE_CONTRACT_SEQUENCE;
}
