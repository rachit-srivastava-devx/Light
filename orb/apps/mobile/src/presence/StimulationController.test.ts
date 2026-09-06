import { describe, expect, it } from 'vitest';

import {
  stimulationForState,
  STIMULATION_CYCLE_MS,
  STIMULATION_ENGAGE_MS,
  STIMULATION_RECOVER_MS,
  voiceEmotionForStimulation,
} from './StimulationController';

describe('bounded stimulation controller', () => {
  it('cycles working energy through engage, recover, and baseline', () => {
    expect(stimulationForState('working', 0)).toMatchObject({ phase: 'engage', level: 0.7 });
    expect(stimulationForState('working', STIMULATION_ENGAGE_MS)).toMatchObject({ phase: 'recover', level: 0.56 });
    expect(stimulationForState('working', STIMULATION_RECOVER_MS)).toMatchObject({ phase: 'baseline', level: 0.52 });
    expect(stimulationForState('working', STIMULATION_CYCLE_MS)).toMatchObject({ phase: 'engage', level: 0.7 });
  });

  it('makes success rewarding but never stimulates degraded or paused states', () => {
    expect(stimulationForState('success', 22_000)).toMatchObject({ phase: 'reward', level: 0.96 });
    expect(stimulationForState('paused', 0)).toMatchObject({ phase: 'baseline', level: 0.16 });
    expect(stimulationForState('error', 1_000)).toMatchObject({ phase: 'baseline', level: 0.28 });
  });

  it('keeps every level within the renderer-safe range', () => {
    for (const state of ['booting', 'listening', 'thinking', 'working', 'success', 'paused', 'error'] as const) {
      for (const elapsedMs of [0, 6_999, 7_000, 16_999, 17_000, 44_999, 45_000]) {
        const model = stimulationForState(state, elapsedMs);
        expect(model.level).toBeGreaterThanOrEqual(0);
        expect(model.level).toBeLessThanOrEqual(1);
      }
    }
  });

  it('raises ordinary task speech briefly, then returns to baseline', () => {
    expect(voiceEmotionForStimulation('warm', stimulationForState('working', 0))).toBe('upbeat');
    expect(voiceEmotionForStimulation('warm', stimulationForState('working', STIMULATION_ENGAGE_MS))).toBe('warm');
    expect(voiceEmotionForStimulation('gentle', stimulationForState('working', 0))).toBe('gentle');
    expect(voiceEmotionForStimulation('warm', stimulationForState('success', 0))).toBe('celebratory');
  });
});
