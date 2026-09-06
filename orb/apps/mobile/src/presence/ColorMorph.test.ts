import { describe, expect, it } from 'vitest';
import {
  BED_STATUS_CUTOFF_HZ,
  COLOR_MORPH_SWEEP_MS,
  THINKING_MORPH_ENGAGE_BUDGET_MS,
  morphTo,
} from './ColorMorph';
import type { AudioParamLike } from './AudioGraph';

function fakeParam(): AudioParamLike & {
  targetCalls: Array<{ target: number; startTime: number; timeConstant: number }>;
} {
  const targetCalls: Array<{ target: number; startTime: number; timeConstant: number }> = [];
  return {
    value: 0,
    targetCalls,
    setValueAtTime() {},
    setTargetAtTime(target, startTime, timeConstant) {
      targetCalls.push({ target, startTime, timeConstant });
    },
    linearRampToValueAtTime() {
      throw new Error('ColorMorph must use setTargetAtTime, not linearRampToValueAtTime');
    },
  };
}

describe('BED_STATUS_CUTOFF_HZ', () => {
  it('orders idle < thinking (brown, low and steady, morphing up toward pink) — §7', () => {
    expect(BED_STATUS_CUTOFF_HZ.idle).toBeLessThan(BED_STATUS_CUTOFF_HZ.thinking);
  });

  it('defines exactly the two colour positions §7 specifies', () => {
    // "Step ready / win" is a gain swell + mixed chime in §7, NOT a third cutoff — modelling it
    // as one would leave the bed permanently brighter after a step, contradicting "idle/working:
    // brown". If a third key ever appears here, check it against §7 first (LESSONS #3).
    expect(Object.keys(BED_STATUS_CUTOFF_HZ).sort()).toEqual(['idle', 'thinking']);
  });

  it('uses the ~400ms sweep §7 specifies', () => {
    expect(COLOR_MORPH_SWEEP_MS).toBe(400);
  });

  it('keeps the revised stop-to-presence scheduling deadline at 250ms', () => {
    expect(THINKING_MORPH_ENGAGE_BUDGET_MS).toBe(250);
  });
});

describe('morphTo', () => {
  it('schedules the filter cutoff toward the target status frequency', () => {
    const freq = fakeParam();
    const end = morphTo(freq, 'thinking', 4, 300);
    expect(freq.targetCalls).toHaveLength(1);
    expect(freq.targetCalls[0]?.target).toBe(BED_STATUS_CUTOFF_HZ.thinking);
    expect(freq.targetCalls[0]?.startTime).toBe(4);
    expect(freq.targetCalls[0]?.timeConstant).toBeCloseTo(0.1, 10);
    expect(end).toBeCloseTo(4.3, 10);
  });

  it('defaults to COLOR_MORPH_SWEEP_MS when no duration is given', () => {
    const freq = fakeParam();
    const end = morphTo(freq, 'idle', 0);
    expect(end).toBeCloseTo(COLOR_MORPH_SWEEP_MS / 1000, 10);
    expect(freq.targetCalls[0]?.target).toBe(BED_STATUS_CUTOFF_HZ.idle);
  });

  it('targets a distinct frequency per status', () => {
    const idle = fakeParam();
    const thinking = fakeParam();
    morphTo(idle, 'idle', 0);
    morphTo(thinking, 'thinking', 0);
    expect(idle.targetCalls[0]?.target).not.toBe(thinking.targetCalls[0]?.target);
  });
});
