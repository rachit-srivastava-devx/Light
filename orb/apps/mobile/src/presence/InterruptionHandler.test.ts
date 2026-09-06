import { describe, expect, it } from 'vitest';
import { classifyInterruption, pauseContractSteps, planRecovery } from './InterruptionHandler';
import { PAUSE_CONTRACT_SEQUENCE, TRANSIENT_RECOVERY_BUDGET_MS } from './contracts';
import type { InterruptionEvent, TransientCause, UserIntentCause } from './contracts';

const TRANSIENT_CAUSES: TransientCause[] = ['incoming_call', 'siri', 'audio_stack_reset'];
const USER_INTENT_CAUSES: UserIntentCause[] = [
  'backgrounded',
  'manual_pause',
  'screen_locked',
  'bluetooth_disconnected',
  'headphone_disconnected',
  'other_app_media',
];

describe('classifyInterruption', () => {
  it.each(TRANSIENT_CAUSES)('classifies %s as transient with automatic-with-cover resume', (cause) => {
    const event: InterruptionEvent = { kind: 'transient', cause, at: 0 };
    const result = classifyInterruption(event);
    expect(result.kind).toBe('transient');
    expect(result.resume).toBe('automatic_with_cover');
  });

  it.each(USER_INTENT_CAUSES)('classifies %s as user_intent with explicit-tap resume', (cause) => {
    const event: InterruptionEvent = { kind: 'user_intent', cause, at: 0 };
    const result = classifyInterruption(event);
    expect(result.kind).toBe('user_intent');
    expect(result.resume).toBe('explicit_user_tap');
    expect(result.cover_required).toBe(false);
  });

  it('requires a cover line only for audio_stack_reset among transient causes (§5)', () => {
    expect(classifyInterruption({ kind: 'transient', cause: 'audio_stack_reset', at: 0 }).cover_required).toBe(
      true,
    );
    expect(classifyInterruption({ kind: 'transient', cause: 'incoming_call', at: 0 }).cover_required).toBe(
      false,
    );
    expect(classifyInterruption({ kind: 'transient', cause: 'siri', at: 0 }).cover_required).toBe(false);
  });

  it('never requires a cover line for a user_intent event', () => {
    for (const cause of USER_INTENT_CAUSES) {
      expect(classifyInterruption({ kind: 'user_intent', cause, at: 0 }).cover_required).toBe(false);
    }
  });
});

describe('planRecovery', () => {
  it('plans ramp_with_cover for an audio-stack reset, within the transient recovery budget', () => {
    const plan = planRecovery({ kind: 'transient', cause: 'audio_stack_reset', at: 1 });
    expect(plan.action).toBe('ramp_with_cover');
    expect(plan.budget_ms).toBe(TRANSIENT_RECOVERY_BUDGET_MS);
  });

  it('plans ramp_and_swell for a call or Siri interruption', () => {
    expect(planRecovery({ kind: 'transient', cause: 'incoming_call', at: 1 }).action).toBe('ramp_and_swell');
    expect(planRecovery({ kind: 'transient', cause: 'siri', at: 1 }).action).toBe('ramp_and_swell');
  });
});

describe('pauseContractSteps', () => {
  it('returns the five steps in the exact §5 order (mic released before UI claims not-listening)', () => {
    const steps = pauseContractSteps();
    expect(steps).toEqual(PAUSE_CONTRACT_SEQUENCE);
    expect(steps.indexOf('release_mic_at_os_level')).toBeLessThan(
      steps.indexOf('show_paused_not_listening'),
    );
    expect(steps.indexOf('bed_fade_out')).toBe(0);
    expect(steps[steps.length - 1]).toBe('capture_nothing');
  });
});
