// F06: the TS half of the lld-ready cross-language comparator (lane contract §7.2). A small,
// dependency-free Node script -- deliberately NOT a new file under orb/ (orb/ is read-only to
// this lane, §5.6) -- that imports the merged, unmodified
// orb/apps/mobile/src/build/LldReadyGate.ts by relative path and emits the same 6-fixture report
// shape fleet/keel/fleet/tests/f06_lld_ready.rs's f06_t10 test emits, so
// lld-ready-crosslang.sh can byte-diff the two.
//
// Runs on Node's own native TypeScript support (confirmed against this repo's installed Node:
// `node <file>.ts` strips types with no flag and no toolchain dependency). orb's own
// node_modules ships no standalone tsx/ts-node binary, only vitest -- and vitest's `include`
// globs (orb/vitest.config.ts) are scoped to files already inside orb/, so pointing it at a
// fleet-owned file would mean editing orb/vitest.config.ts, which this lane may not do (§5.6:
// every file under orb/ is read-only). Node's native loader sidesteps that entirely.
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { lldReady } from '../../../orb/apps/mobile/src/build/LldReadyGate.ts';

const HERE = dirname(fileURLToPath(import.meta.url));
const FIXTURES_DIR = join(HERE, '..', '..', 'contracts', 'fixtures', 'lld');

// Mirrors fleet/contracts/owners.v1.json + fleet/contracts/gate-refs.v1.json. Read as data here
// (not imported as code) so this script has zero dependency beyond node:fs/node:path/node:url.
const OWNERS_JSON = JSON.parse(
  readFileSync(join(HERE, '..', '..', 'contracts', 'owners.v1.json'), 'utf8'),
);
const GATE_REFS_JSON = JSON.parse(
  readFileSync(join(HERE, '..', '..', 'contracts', 'gate-refs.v1.json'), 'utf8'),
);
const REFS = {
  owners: OWNERS_JSON.owners as string[],
  registry_paths: GATE_REFS_JSON.registry_paths as string[],
  known_node_ids: GATE_REFS_JSON.known_node_ids as string[],
};

// Sorted -- matches f06_t10_rust_and_ts_gates_agree_on_every_fixture's FIXTURE_NAMES and
// lld.rs's cross_lang_report convention of a sorted fixture-name key order.
const FIXTURE_NAMES = [
  'complete_freeze',
  'complete_module',
  'forged_freeze',
  'numeric_trap',
  'one_alternative',
  'one_line_freeze',
];

function briefOf(parsed: unknown): unknown {
  if (parsed && typeof parsed === 'object' && 'module_brief' in (parsed as Record<string, unknown>)) {
    return (parsed as Record<string, unknown>).module_brief;
  }
  return parsed;
}

const fixtures: Record<string, { failed_check_ids: string[]; outcome: string }> = {};
for (const name of FIXTURE_NAMES) {
  const raw = JSON.parse(readFileSync(join(FIXTURES_DIR, `${name}.json`), 'utf8'));
  const brief = briefOf(raw);
  // eslint-disable-next-line @typescript-eslint/no-explicit-any -- crossing the fixture/gate
  // boundary with raw JSON, exactly like the Rust and F02 mirrors do.
  const verdict: any = lldReady(brief as any, REFS as any);
  const failed: string[] = (verdict.reasons ?? []).map((r: { check_id: string }) => r.check_id).sort();
  // Key order (failed_check_ids before outcome) matches Rust's BTreeMap-backed serde_json::Map,
  // which sorts alphabetically regardless of insertion order; this side has no such guarantee,
  // so the literal's key order is written to match on purpose.
  fixtures[name] = { failed_check_ids: failed, outcome: verdict.outcome };
}

// Top-level key order (fixtures, gate, mirror) is alphabetical, again to match the Rust side.
const report = {
  fixtures,
  gate: 'lld-ready',
  mirror: 'ts',
};

const out = process.env.F06_REPORT_OUT ?? '/dev/null';
writeFileSync(out, JSON.stringify(report, null, 2));
