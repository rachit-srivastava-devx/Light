import { describe, expect, it } from 'vitest';

import { INTERVENTION_BUDGET_PER_SESSION, type PolicyDecision } from '../cognitive/contracts';
import { armCheckInTimer, checkInDue } from './CheckInTimer';
import type { SessionSnapshot } from './contracts';

const NOW = 1_700_000_000_000;

const working: SessionSnapshot = {
  session_id: 's1',
  state: 'WORKING',
  step_index: 1,
  steps_total: 3,
  interrupted_from: null,
  bed_active: true,
};

const policy: PolicyDecision = {
  intervention: 'Suggest',
  severity: 'normal',
  reason: 'guard.overwhelmed',
  refractory_until_ts: NOW + 90_000,
  budget_remaining: INTERVENTION_BUDGET_PER_SESSION - 1,
};

describe('CheckInTimer', () => {
  it('arms only after a real policy intervention while WORKING', () => {
    const r = armCheckInTimer(working, policy, NOW, 500);
    expect(r.kind).toBe('armed');
    if (r.kind === 'armed') {
      expect(r.timer).toEqual({
        session_id: 's1',
        step_index: 1,
        due_ts: NOW + 500,
        reason: 'guard.overwhelmed',
      });
    }
  });

  it('does not arm for Observe, because silence is not a check-in request', () => {
    const r = armCheckInTimer(working, { ...policy, intervention: 'Observe' }, NOW, 500);
    expect(r).toEqual({ kind: 'not_armed', reason: 'observe_only' });
  });

  it('does not arm outside WORKING', () => {
    const r = armCheckInTimer({ ...working, state: 'CHECK_IN' }, policy, NOW, 500);
    expect(r).toEqual({ kind: 'not_armed', reason: 'not_working' });
  });

  it('rejects invalid delays and missing step context', () => {
    expect(armCheckInTimer(working, policy, NOW, Number.NaN)).toEqual({
      kind: 'not_armed',
      reason: 'invalid_delay',
    });
    expect(armCheckInTimer({ ...working, step_index: 0 }, policy, NOW, 500)).toEqual({
      kind: 'not_armed',
      reason: 'missing_step',
    });
  });

  it('fires only for the same session and step after the due time', () => {
    const r = armCheckInTimer(working, policy, NOW, 500);
    if (r.kind !== 'armed') throw new Error('fixture should arm');
    expect(checkInDue(r.timer, working, NOW + 499)).toBe(false);
    expect(checkInDue(r.timer, working, NOW + 500)).toBe(true);
    expect(checkInDue(r.timer, { ...working, step_index: 2 }, NOW + 500)).toBe(false);
  });
});
