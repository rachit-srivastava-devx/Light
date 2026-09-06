import { describe, expect, it } from 'vitest';
import { BUILD_TRANSITION_TABLE, buildDispatch } from './BuildStateMachine';
import type { BuildState } from './contracts';
import { SESSION_TRANSITION_TABLE } from '../session/StateMachine';

describe('build-mode FSM', () => {
  it('U2-T1 FROZEN is reachable only from FREEZE_CONFIRM on human_confirm', () => {
    const intoFrozen: string[] = [];
    for (const [from, edges] of Object.entries(BUILD_TRANSITION_TABLE)) {
      for (const [event, t] of Object.entries(edges as Record<string, any>)) {
        const targets = t.kind === 'branch' ? [t.when_true, t.when_false] : [t.to];
        if (targets.includes('FROZEN')) intoFrozen.push(`${from}:${event}`);
      }
    }
    expect(intoFrozen).toEqual(['FREEZE_CONFIRM:human_confirm']);
  });

  it('U2-T2 FREEZE_CONFIRM is only entered behind the gate_verdict_ready guard', () => {
    for (const [from, edges] of Object.entries(BUILD_TRANSITION_TABLE)) {
      for (const [event, t] of Object.entries(edges as Record<string, any>)) {
        if (t.kind === 'to' && t.to === 'FREEZE_CONFIRM') {
          expect(t.guard, `${from}:${event}`).toBe('gate_verdict_ready');
        }
      }
    }
  });

  it('U2-T3 a false gate guard cannot advance out of PROPOSE', () => {
    expect(buildDispatch('PROPOSE', 'gate_ready', { gate_ready: false })).toBe('PROPOSE');
    expect(buildDispatch('PROPOSE', 'gate_ready', { gate_ready: true })).toBe('FREEZE_CONFIRM');
  });

  it('U2-T4 build and session state spaces are disjoint', () => {
    const build = new Set(Object.keys(BUILD_TRANSITION_TABLE));
    const session = new Set(Object.keys(SESSION_TRANSITION_TABLE));
    expect([...build].filter((s) => session.has(s))).toEqual([]);
  });

  it('U2-T5 dispatch is clock-free and replay-identical', () => {
    const path: Array<[BuildState, any]> = [
      ['BUILD_IDLE', 'enter_build'], ['BUILD_INTAKE', 'goal_captured'],
      ['DECOMPOSE', 'brief_drafted'], ['PROPOSE', 'gate_ready'],
    ];
    const run = () => path.map(([s, e]) =>
      buildDispatch(s, e, { gate_ready: true, brief_valid: true })).join('>');
    const now = Date.now; const rand = Math.random;
    (Date as any).now = () => { throw new Error('clock read in dispatch'); };
    (Math as any).random = () => { throw new Error('randomness in dispatch'); };
    try { expect(run()).toBe(run()); } finally { (Date as any).now = now; (Math as any).random = rand; }
  });
});
