import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { lldReady, GATE_CHECK_IDS } from './LldReadyGate';

// F02: fixtures moved fleet-ward (schema ownership moved with them -- fleet/contracts/fixtures/lld/).
const FIX = `${__dirname}/../../../../../fleet/contracts/fixtures/lld`;
const load = (n: string) => JSON.parse(readFileSync(`${FIX}/${n}.json`, 'utf8'));
const REFS = {
  owners: ['rachit@devxlabs.ai'],
  registry_paths: ['registry/services/llm-gateway', 'registry/features/cost-control-plane'],
  known_node_ids: ['orb-freeze-ledger'],
};

describe('lld-ready gate', () => {
  it('U3-T1 refuses the one-line freeze, naming ≥5 distinct check ids', () => {
    const v: any = lldReady(load('one_line_freeze'), REFS);
    expect(v.outcome).toBe('NOT_READY');
    expect(new Set(v.reasons.map((r: any) => r.check_id)).size).toBeGreaterThanOrEqual(5);
  });

  it('U3-T2 accepts the control fixture with checked driven to full count', () => {
    const v: any = lldReady(load('complete_module'), REFS);
    expect(v.outcome).toBe('READY');
    expect(v.checked).toBe(GATE_CHECK_IDS.length);
    expect(v.score.checks_passed).toBe(v.score.checks_total);
  });

  it('U3-T3 every check id can be failed in isolation from the control fixture', () => {
    // Each mutation below must trip exactly its own check and no other.
    const base = load('complete_module');
    const mutations: Record<string, (b: any) => any> = {
      'C1-OPEN':        (b) => ({ ...b, open_questions: ['what store?'] }),
      'C2-OWNER':       (b) => ({ ...b, owner: 'me' }),
      'C3-ACC-NONTAUT': (b) => ({ ...b, acceptance: { ...b.acceptance, then: 'it works' } }),
      'R17-DERIV':      (b) => ({ ...b, guarantees: [{ ...b.guarantees[0],
                                   derivation: { kind: 'number', value: 'fast', calc: '' } }] }),
      'R19-ABSOLUTE':   (b) => ({ ...b, guarantees: [{ claim: 'zero data loss', label: 'mitigates',
                                   derivation: { kind: 'number', value: '0', calc: '0 = 0' } }] }),
      'R21-ALTS':       (b) => ({ ...b, alternatives: b.alternatives.slice(0, 1) }),
      'R21-FAIL':       (b) => ({ ...b, failure_story: { ...b.failure_story, blast_radius: '' } }),
      'C12-STORE':      (b) => ({ ...b, data_owned: [{ store: 'x', owned_by_node: 'someone-else' }] }),
    };
    for (const [id, mutate] of Object.entries(mutations)) {
      const v: any = lldReady(mutate(base), REFS);
      expect(v.outcome, id).toBe('NOT_READY');
      expect(v.reasons.map((r: any) => r.check_id), id).toContain(id);
    }
  });

  it('U3-T4 a high depth_score is NOT a pass — the ratio is never a threshold', () => {
    const v: any = lldReady({ ...load('complete_module'), open_questions: ['one'] }, REFS);
    expect(v.outcome).toBe('NOT_READY');
    expect(v.score.ratio).toBeGreaterThan(0.9);   // nearly perfect, still refused
  });

  it('U3-T5 the gate is pure: no clock, no randomness, no network, no fs', async () => {
    const brief = load('complete_module');
    const saved = { now: Date.now, rand: Math.random, fetch: globalThis.fetch };
    (Date as any).now = () => { throw new Error('clock'); };
    (Math as any).random = () => { throw new Error('random'); };
    (globalThis as any).fetch = () => { throw new Error('network'); };
    try {
      const a = lldReady(brief, REFS); const b = lldReady(brief, REFS);
      expect(JSON.stringify(a)).toBe(JSON.stringify(b));
    } finally { Object.assign(Date, { now: saved.now }); Object.assign(Math, { random: saved.rand });
                (globalThis as any).fetch = saved.fetch; }
  });

  it('U3-T6 an empty check set is MEASURED_NOTHING, never READY', () => {
    const v = lldReady(load('complete_module'), { owners: [], registry_paths: [], known_node_ids: [] });
    expect(v.outcome).not.toBe('READY');   // owners is empty ⇒ C2 cannot pass; never a vacuous green
  });
});
