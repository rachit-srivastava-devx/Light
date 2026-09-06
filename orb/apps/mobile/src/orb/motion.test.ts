import { describe, expect, it } from 'vitest';

import {
  BREATH_PULSE_PERIOD_MS,
  MIST_DRIFT_RADIUS_FRACTION_MAX,
  PULSE_OPACITY_DEPTH,
  PULSE_OPACITY_FLOOR,
  PULSE_SCALE_DEPTH,
  breathsPerMinuteToPeriodMs,
  effectiveBreathDepth,
  effectiveMistSpeed,
  pulseOpacityRange,
  pulseScaleRange,
  resolvePulseMotionState,
} from './motion';

describe('breathsPerMinuteToPeriodMs', () => {
  it('converts a breathing rate to a full-cycle period', () => {
    expect(breathsPerMinuteToPeriodMs(6)).toBe(10_000);
    expect(breathsPerMinuteToPeriodMs(60)).toBe(1_000);
    expect(breathsPerMinuteToPeriodMs(4.5)).toBeCloseTo(13_333.33, 1);
  });

  it('is the exact provenance of BREATH_PULSE_PERIOD_MS (6 breaths/min)', () => {
    expect(BREATH_PULSE_PERIOD_MS).toBe(breathsPerMinuteToPeriodMs(6));
  });

  it('is dramatically slower than the pre-fix 1400ms decorative tick (~43 cycles/min)', () => {
    // Regression guard for defect #2: the old period was not a physiologically possible breath
    // rate. Anything under 5x slower would put us back in "fast decorative tick" territory.
    const OLD_PULSE_PERIOD_MS = 1_400;
    expect(BREATH_PULSE_PERIOD_MS).toBeGreaterThanOrEqual(OLD_PULSE_PERIOD_MS * 5);
  });
});

describe('resolvePulseMotionState', () => {
  it('is "off" whenever the mic is not listening, regardless of the reduce-motion setting', () => {
    expect(resolvePulseMotionState({ micListening: false, reduceMotionEnabled: false })).toBe('off');
    expect(resolvePulseMotionState({ micListening: false, reduceMotionEnabled: true })).toBe('off');
  });

  it('animates while listening when the OS has not requested reduced motion', () => {
    expect(resolvePulseMotionState({ micListening: true, reduceMotionEnabled: false })).toBe('animating');
  });

  it('freezes (does not turn off) while listening when reduced motion is requested', () => {
    // Frozen, not "off": the mic-listening signal should stay legible even with motion reduced.
    expect(resolvePulseMotionState({ micListening: true, reduceMotionEnabled: true })).toBe('frozen');
  });
});

describe('pulseOpacityRange', () => {
  it('floors low opacity states up to PULSE_OPACITY_FLOOR, matching the original Math.max shape', () => {
    expect(pulseOpacityRange(0.2)).toEqual({
      low: PULSE_OPACITY_FLOOR,
      high: PULSE_OPACITY_FLOOR + PULSE_OPACITY_DEPTH,
    });
  });

  it('lets a base opacity between the floor and the ceiling raise only the low end', () => {
    const base = PULSE_OPACITY_FLOOR + 0.1;
    expect(pulseOpacityRange(base)).toEqual({
      low: base,
      high: PULSE_OPACITY_FLOOR + PULSE_OPACITY_DEPTH,
    });
  });

  it('collapses to a constant (no visible pulse) once base opacity already exceeds the ceiling', () => {
    const base = PULSE_OPACITY_FLOOR + PULSE_OPACITY_DEPTH + 0.1;
    const range = pulseOpacityRange(base);
    expect(range.low).toBe(base);
    expect(range.high).toBe(base);
  });
});

describe('pulseScaleRange', () => {
  it('oscillates between 1 (no growth) and 1 + PULSE_SCALE_DEPTH', () => {
    expect(pulseScaleRange()).toEqual({ low: 1, high: 1 + PULSE_SCALE_DEPTH });
  });
});

describe('effectiveMistSpeed / effectiveBreathDepth (reduced-motion mitigation passed to AuroraMist)', () => {
  it('passes the base value through unchanged when reduce-motion is off', () => {
    expect(effectiveMistSpeed(7, false)).toBe(7);
    expect(effectiveBreathDepth(0.08, false)).toBe(0.08);
  });

  it('freezes both to zero when reduce-motion is on, regardless of the base value', () => {
    expect(effectiveMistSpeed(7, true)).toBe(0);
    expect(effectiveMistSpeed(0.001, true)).toBe(0);
    expect(effectiveBreathDepth(0.08, true)).toBe(0);
  });
});

describe('MIST_DRIFT_RADIUS_FRACTION_MAX', () => {
  it('is well under the original ~25%-of-radius excursion AuroraMist.tsx currently hardcodes', () => {
    // Regression guard for defect #3: this constant only has teeth once AuroraMist.tsx imports it
    // (tracked separately — different file owner), but if this file's own number silently creeps
    // back toward 0.25 without revisiting the evidence in motion.ts, this test should catch it.
    const ORIGINAL_AURORA_MIST_DRIFT_FRACTION = 0.25;
    expect(MIST_DRIFT_RADIUS_FRACTION_MAX).toBeLessThan(ORIGINAL_AURORA_MIST_DRIFT_FRACTION / 2);
    expect(MIST_DRIFT_RADIUS_FRACTION_MAX).toBeGreaterThan(0);
  });
});
