/**
 * Policy-invoked check-in timer (`docs/BUILD-DIGEST.md` §1).
 *
 * The timer is mechanical: it can move WORKING -> CHECK_IN only after the policy has already
 * emitted an admissible intervention. It never infers completion, never starts from wall-clock
 * time internally, and never runs as a fixed background nag.
 */

import type { PolicyDecision } from '../cognitive/contracts';
import type { EpochMs, SessionSnapshot } from './contracts';

export interface CheckInTimer {
  readonly session_id: string;
  readonly step_index: number;
  readonly due_ts: EpochMs;
  readonly reason: PolicyDecision['reason'];
}

export type CheckInArmResult =
  | { readonly kind: 'armed'; readonly timer: CheckInTimer }
  | {
      readonly kind: 'not_armed';
      readonly reason: 'not_working' | 'observe_only' | 'invalid_delay' | 'missing_step';
    };

export function armCheckInTimer(
  session: SessionSnapshot,
  policy: PolicyDecision,
  now: EpochMs,
  delay_ms: number,
): CheckInArmResult {
  if (session.state !== 'WORKING') return { kind: 'not_armed', reason: 'not_working' };
  if (policy.intervention === 'Observe') return { kind: 'not_armed', reason: 'observe_only' };
  if (!Number.isFinite(delay_ms) || delay_ms < 0) return { kind: 'not_armed', reason: 'invalid_delay' };
  if (!Number.isInteger(session.step_index) || session.step_index < 1) {
    return { kind: 'not_armed', reason: 'missing_step' };
  }

  return {
    kind: 'armed',
    timer: {
      session_id: session.session_id,
      step_index: session.step_index,
      due_ts: now + delay_ms,
      reason: policy.reason,
    },
  };
}

export function checkInDue(timer: CheckInTimer, session: SessionSnapshot, now: EpochMs): boolean {
  return (
    session.state === 'WORKING' &&
    session.session_id === timer.session_id &&
    session.step_index === timer.step_index &&
    now >= timer.due_ts
  );
}
