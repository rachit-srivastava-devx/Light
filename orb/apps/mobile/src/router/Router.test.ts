import { describe, expect, it } from 'vitest';
import { route } from './Router';
import { STUCK_REATOMIZE_THRESHOLD } from './contracts';
import type { IntentLabel } from './contracts';
import type { SessionState } from '../session/contracts';

const NON_INTAKE_STATE: SessionState = 'WORKING';

// One test per row of the §2 lookup table.
describe('Router.route — one row per digest table entry', () => {
  it('intent==done -> advance_step', () => {
    expect(route(NON_INTAKE_STATE, 'done', 0)).toBe('advance_step');
  });

  it('intent==next -> advance_step', () => {
    expect(route(NON_INTAKE_STATE, 'next', 0)).toBe('advance_step');
  });

  it('intent==pause -> pause_session', () => {
    expect(route(NON_INTAKE_STATE, 'pause', 0)).toBe('pause_session');
  });

  it('intent==stuck AND stuck_count < STUCK_REATOMIZE_THRESHOLD -> reanchor_templated', () => {
    expect(route(NON_INTAKE_STATE, 'stuck', STUCK_REATOMIZE_THRESHOLD - 1)).toBe(
      'reanchor_templated',
    );
    expect(route(NON_INTAKE_STATE, 'stuck', 0)).toBe('reanchor_templated');
  });

  it('intent==stuck AND stuck_count >= STUCK_REATOMIZE_THRESHOLD -> crew_reatomize', () => {
    expect(route(NON_INTAKE_STATE, 'stuck', STUCK_REATOMIZE_THRESHOLD)).toBe('crew_reatomize');
    expect(route(NON_INTAKE_STATE, 'stuck', STUCK_REATOMIZE_THRESHOLD + 5)).toBe('crew_reatomize');
  });

  it('intent==question -> fast_voice', () => {
    expect(route(NON_INTAKE_STATE, 'question', 0)).toBe('fast_voice');
  });

  it('intent==chitchat -> fast_voice', () => {
    expect(route(NON_INTAKE_STATE, 'chitchat', 0)).toBe('fast_voice');
  });

  it('state==INTAKE -> crew_atomize, regardless of intent (see Router.ts judgment-call note)', () => {
    const allIntents: readonly IntentLabel[] = [
      'done',
      'next',
      'stuck',
      'pause',
      'question',
      'chitchat',
    ];
    for (const intent of allIntents) {
      expect(route('INTAKE', intent, 0)).toBe('crew_atomize');
    }
  });

  it('is 100% deterministic: same inputs always produce the same action', () => {
    for (let i = 0; i < 20; i++) {
      expect(route('WORKING', 'stuck', 3)).toBe('crew_reatomize');
    }
  });
});
