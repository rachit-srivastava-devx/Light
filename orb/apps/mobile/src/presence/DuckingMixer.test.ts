import { describe, expect, it } from 'vitest';
import { duckBed, restoreBed } from './DuckingMixer';
import { dbfsToLinearGain } from './AudioGraph';
import type { AudioParamLike } from './AudioGraph';
import { BED_GAIN_DUCKED_DBFS, BED_GAIN_NOMINAL_DBFS, DUCK_RAMP_MAX_MS, DUCK_RAMP_MIN_MS } from './contracts';

/**
 * Blueprint 02 §4 names `setTargetAtTime` for the duck. It also forbids, structurally, any
 * absolute set on this param: `setValueAtTime` would snap the gain from wherever an in-flight ramp
 * really is to an assumed value — a click on a bed that must never draw attention to itself. Both
 * of the other primitives therefore throw here (LESSONS #2).
 */
function fakeParam(): AudioParamLike & {
  targetCalls: Array<{ target: number; startTime: number; timeConstant: number }>;
} {
  const targetCalls: Array<{ target: number; startTime: number; timeConstant: number }> = [];
  return {
    value: 0,
    targetCalls,
    setValueAtTime() {
      throw new Error('duck/restore must not snap the bed gain with setValueAtTime (§4)');
    },
    setTargetAtTime(target, startTime, timeConstant) {
      targetCalls.push({ target, startTime, timeConstant });
    },
    linearRampToValueAtTime() {
      throw new Error('duck/restore must use setTargetAtTime per blueprint 02 §4');
    },
  };
}

describe('duckBed', () => {
  it('targets the ducked dBFS with setTargetAtTime, completing within the 80-150ms window', () => {
    const gain = fakeParam();
    const end = duckBed(gain, 10, 120);

    expect(gain.targetCalls).toEqual([
      { target: dbfsToLinearGain(BED_GAIN_DUCKED_DBFS), startTime: 10, timeConstant: 0.12 / 3 },
    ]);
    expect(end).toBeCloseTo(10.12, 10);
  });

  it('clamps a ramp duration below the 80ms floor', () => {
    const gain = fakeParam();
    const end = duckBed(gain, 0, 10);
    expect(end).toBeCloseTo(DUCK_RAMP_MIN_MS / 1000, 10);
    expect(gain.targetCalls[0]?.timeConstant).toBeCloseTo(DUCK_RAMP_MIN_MS / 1000 / 3, 10);
  });

  it('clamps a ramp duration above the 150ms ceiling', () => {
    const gain = fakeParam();
    const end = duckBed(gain, 0, 10_000);
    expect(end).toBeCloseTo(DUCK_RAMP_MAX_MS / 1000, 10);
    expect(gain.targetCalls[0]?.timeConstant).toBeCloseTo(DUCK_RAMP_MAX_MS / 1000 / 3, 10);
  });

  it('never reads or asserts a current value — overlapping ducks cannot click', () => {
    const gain = fakeParam();
    // back-to-back TTS chunks: duck, restore, duck again mid-restore. No throw = no absolute set.
    expect(() => {
      duckBed(gain, 0);
      restoreBed(gain, 0.2);
      duckBed(gain, 0.25);
    }).not.toThrow();
    expect(gain.targetCalls).toHaveLength(3);
  });
});

describe('restoreBed', () => {
  it('targets nominal dBFS with the same primitive and window', () => {
    const gain = fakeParam();
    const end = restoreBed(gain, 5, 100);

    expect(gain.targetCalls).toEqual([
      { target: dbfsToLinearGain(BED_GAIN_NOMINAL_DBFS), startTime: 5, timeConstant: 0.1 / 3 },
    ]);
    expect(end).toBeCloseTo(5.1, 10);
  });
});
