import { describe, expect, it } from 'vitest';
import { readFileSync, writeFileSync } from 'node:fs';
import { validateModuleBrief, validateLldV1, canonicalJson, contentHash, crossLangReport } from './lld-v1';

// F02: fixtures moved fleet-ward (schema ownership moved with them -- the lld-ready gate is
// fleet's, F02 lane contract §3.1). `load()` is repointed at the fleet corpus; U1-T1..U1-T5 below
// are otherwise byte-unchanged.
const FIX = `${__dirname}/../../../../../fleet/contracts/fixtures/lld`;
const load = (n: string) => JSON.parse(readFileSync(`${FIX}/${n}.json`, 'utf8'));

describe('lld.v1 schema', () => {
  it('U1-T1 accepts the complete_module fixture', () => {
    expect(validateModuleBrief(load('complete_module')).ok).toBe(true);
  });

  it('U1-T2 rejects the one_line_freeze fixture, naming ≥3 distinct field paths', () => {
    const r = validateModuleBrief(load('one_line_freeze'));
    expect(r.ok).toBe(false);
    expect(new Set(r.errors.map((e) => e.path)).size).toBeGreaterThanOrEqual(3);
  });

  it('U1-T3 has no proposer-writable authority field', () => {
    // ModuleBrief must not carry any field the gate is supposed to stamp.
    const brief = load('complete_module');
    for (const forbidden of ['content_hash', 'freeze_id', 'depth_score', 'stamped_by', 'state', 'version']) {
      expect(Object.prototype.hasOwnProperty.call(brief, forbidden)).toBe(false);
    }
    // ...and adding one must be a validation error, not a silently ignored extra key.
    expect(validateModuleBrief({ ...brief, stamped_by: 'orb:lld-ready' }).ok).toBe(false);
  });

  it('U1-T4 content_hash is invariant to narration/register/ordering', () => {
    const brief = load('complete_module');
    const reordered = Object.fromEntries(Object.entries(brief).reverse());
    const narrated = { ...brief, purpose: brief.purpose };            // same decision, same bytes
    expect(contentHash(reordered)).toBe(contentHash(brief));
    expect(contentHash(narrated)).toBe(contentHash(brief));
    // and it MUST change when a decision changes
    expect(contentHash({ ...brief, owner_path: 'apps/other/' })).not.toBe(contentHash(brief));
  });

  it('U1-T5 canonicalJson is stable across 100 shuffles', () => {
    const brief = load('complete_module');
    const hashes = new Set(
      Array.from({ length: 100 }, () =>
        contentHash(Object.fromEntries(
          Object.entries(brief).sort(() => Math.random() - 0.5)))),
    );
    expect(hashes.size).toBe(1);
  });

  it('F02-T6 rejects a leaf-grain brief with only one killed alternative', () => {
    const r = validateModuleBrief(load('one_alternative'));
    expect(r.ok).toBe(false);
    expect(r.errors.map((e) => e.path)).toContain('alternatives');
  });

  it('F02-T7 rejects a freeze stamped by the proposer', () => {
    const r = validateLldV1(load('forged_freeze'));
    expect(r.ok).toBe(false);
    expect(r.errors.map((e) => e.path)).toContain('freeze.stamped_by');
  });

  it('F02-T8 accepts only keel:lld-ready, and constructs it nowhere', () => {
    const ok = load('complete_freeze');
    expect(ok.freeze.stamped_by).toBe('keel:lld-ready');
    // structural: no orb source file may contain the stamper literal.
    const src = readFileSync(`${__dirname}/lld-v1.ts`, 'utf8');
    expect(src.includes("'keel:lld-ready'")).toBe(false);   // type-level const only, never a value
  });

  it('F02-T9 canonicalJson REFUSES a numeric leaf (the cross-language encoding trap)', () => {
    expect(() => canonicalJson({ ratio: 1.0 })).toThrow(/numbers are not canonicalizable/);
    expect(() => contentHash(load('numeric_trap'))).toThrow(/numbers are not canonicalizable/);
    // and the guard is not vacuous: the good fixture still hashes.
    expect(contentHash(load('complete_module'))).toMatch(/^sha256:[0-9a-f]{64}$/);
  });

  it('F02-T10 content_hash carries its algorithm', () => {
    expect(contentHash(load('complete_module'))).toMatch(/^sha256:[0-9a-f]{64}$/);
  });

  it('F02-T11 a freeze self-verifies against its own brief', () => {
    const w = load('complete_freeze');
    expect(w.freeze.content_hash).toBe(contentHash(w.module_brief));
  });

  it('F02-T12 emits the cross-language report (consumed by lld-crosslang.sh)', () => {
    writeFileSync(process.env.F02_REPORT_OUT ?? '/dev/null', JSON.stringify(crossLangReport(FIX), null, 2));
  });
});
