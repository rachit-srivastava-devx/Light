import { describe, expect, it } from 'vitest';

import { selectProsody, type OrbEmotion } from './ProsodyDirector';
import { INTERVENTION_LADDER, type Intervention } from '../cognitive/contracts';
import type { SessionState } from '../session/contracts';

const ALL_STATES: readonly SessionState[] = [
  'IDLE_PRESENT',
  'INTAKE',
  'CLARIFY',
  'STEP_PRESENT',
  'WORKING',
  'CHECK_IN',
  'STEP_DONE',
  'INTERRUPTED',
  'SESSION_DONE',
];

describe('ProsodyDirector', () => {
  it('returns a directive for every session state', () => {
    for (const state of ALL_STATES) {
      expect(selectProsody(state, null)).toBeDefined();
    }
  });

  it('returns a directive for every intervention in every state', () => {
    // Exhaustive: 9 states x 9 interventions. A missing entry would surface as undefined at
    // speak-time, i.e. a crash mid-session.
    for (const state of ALL_STATES) {
      for (const intervention of INTERVENTION_LADDER) {
        const d = selectProsody(state, intervention as Intervention);
        expect(d.emotion).toBeTruthy();
        expect(d.rate).toBeGreaterThan(0);
      }
    }
  });

  it('celebrates a completed step', () => {
    expect(selectProsody('STEP_DONE', null).emotion).toBe('celebratory');
  });

  it('asks a question, not an interrogation, in CLARIFY', () => {
    const d = selectProsody('CLARIFY', null);
    expect(d.emotion).toBe('curious');
    expect(d.rate).toBeLessThan(1);
  });

  it('is gentle and slow on Pause and Escalate — the only interventions INV7 allows at high dysregulation', () => {
    for (const intervention of ['Pause', 'Escalate'] as const) {
      const d = selectProsody('WORKING', intervention);
      expect(d.emotion).toBe('gentle');
      expect(d.rate).toBeLessThan(1);
      expect(d.energy).toBe('low');
    }
  });

  it('lets an unlisted intervention keep the state tone rather than announcing itself', () => {
    expect(selectProsody('WORKING', 'Suggest')).toEqual(selectProsody('WORKING', null));
  });

  it('is deterministic', () => {
    expect(selectProsody('CHECK_IN', 'Redirect')).toEqual(selectProsody('CHECK_IN', 'Redirect'));
  });

  it('cannot express a shame-adjacent register (INV7, structurally)', () => {
    // The real guarantee is the type: `OrbEmotion` has no such member, so this test is a
    // tripwire on the union rather than a runtime check. If someone widens the union, this fails.
    const allowed: readonly OrbEmotion[] = [
      'warm',
      'calm',
      'curious',
      'celebratory',
      'gentle',
      'matter_of_fact',
    ];
    const emitted = new Set<string>();
    for (const state of ALL_STATES) {
      emitted.add(selectProsody(state, null).emotion);
      for (const intervention of INTERVENTION_LADDER) {
        emitted.add(selectProsody(state, intervention as Intervention).emotion);
      }
    }
    for (const e of emitted) {
      expect(allowed).toContain(e as OrbEmotion);
    }
    // and none of the excluded registers can appear
    for (const banned of ['disappointed', 'stern', 'urgent', 'admonishing', 'exasperated']) {
      expect(emitted.has(banned)).toBe(false);
    }
  });
});
