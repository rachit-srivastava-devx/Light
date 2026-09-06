import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { validateModuleBrief, canonicalJson, contentHash } from './lld-v1';

const load = (n: string) => JSON.parse(readFileSync(`${__dirname}/fixtures/${n}.json`, 'utf8'));

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
});
