/**
 * The `lld-ready` gate — SPEED-OF-THOUGHT-P0-CONTRACT.md §3.
 *
 * Deterministic, pure pass/fail rules over a `ModuleBrief`. Reports `depth_score` but never uses
 * it as a pass criterion (§3.4) — see U3-T4: a brief scoring 13/14 (every structural check but one)
 * must still be `NOT_READY`. A ratio threshold would let a proposer trade a real acceptance test
 * for two padded alternatives; the cheapest green would stop being the correct fix.
 *
 * Purity (U3-T5): no `Date.now`, no `Math.random`, no network, no filesystem access inside this
 * module. All external state (`GateRefs`) is passed in by the caller.
 *
 * Reconciliation note: this gate originally imported `ModuleBrief` from a local placeholder mirror
 * (`./lld-v1-local.ts`) because U1 (`./lld-v1.ts`) had not yet merged. U1 merged at `79652d9` with
 * field names identical to the contract doc, so this is now a one-line repoint to the real module —
 * see FLEET-LEARNINGS.md's 2026-09-02 reconciliation entry for this branch.
 */
import type { KilledAlt, ModuleBrief } from './lld-v1';

export const GATE_CHECK_IDS = [
  'C1-OPEN',
  'C2-OWNER',
  'C3-ACC-PARSE',
  'C3-ACC-GROUND',
  'C3-ACC-NONTAUT',
  'R17-DERIV',
  'R19-ABSOLUTE',
  'R21-ALTS',
  'R21-FAIL',
  'C12-STORE',
  'C12-DEPS',
  'REG-VERDICT',
  'IFACE',
  'SHAPE',
] as const;

export type GateCheckId = (typeof GATE_CHECK_IDS)[number];

export interface GateRefs {
  readonly owners: readonly string[];
  readonly registry_paths: readonly string[];
  readonly known_node_ids: readonly string[];
}

export interface GateReason {
  readonly check_id: GateCheckId;
  readonly detail: string;
}

export interface DepthScore {
  readonly checks_total: number;
  readonly checks_passed: number;
  readonly ratio: number;
  readonly failed_check_ids: readonly string[];
}

export type GateVerdict =
  | { readonly outcome: 'READY'; readonly checked: number; readonly score: DepthScore }
  | { readonly outcome: 'NOT_READY'; readonly checked: number; readonly score: DepthScore;
      readonly reasons: readonly GateReason[] }
  | { readonly outcome: 'MEASURED_NOTHING'; readonly checked: 0 };

const TAUTOLOGY_DENYLIST: readonly RegExp[] = [
  /^(it )?works?$/i,
  /^(is )?correct$/i,
  /^(it )?should work/i,
  /^passes( the)? tests?$/i,
  /^no errors?$/i,
];

const NON_TRIGGER_DENYLIST = /^(tbd|n\/a|none|-|\?)$/i;
const ABSOLUTE_CLAIM = /\b(zero|never|impossible|cannot|no way)\b/i;
const ENFORCED_BY_PATTERN = /[A-Za-z0-9_\-/.]+\.(ts|tsx|py|rs|json)(:[0-9]+)?$/;
const NODE_ID_PATTERN = /^[a-z0-9][a-z0-9-]{2,63}$/;
const CALC_ARITH = /[+\-*/÷×=≈%]/;

function trimLen(s: string): number {
  return s.trim().length;
}

function checkC1Open(b: ModuleBrief): boolean {
  return b.open_questions.length === 0;
}

function checkC2Owner(b: ModuleBrief, refs: GateRefs): boolean {
  return refs.owners.includes(b.owner);
}

function checkC3AccParse(b: ModuleBrief): boolean {
  const a = b.acceptance;
  return (
    trimLen(a.given) >= 3 &&
    trimLen(a.when) >= 3 &&
    trimLen(a.then) >= 3 &&
    (a.oracle_kind === 'test' || a.oracle_kind === 'property' || a.oracle_kind === 'metric')
  );
}

function checkC3AccGround(b: ModuleBrief): boolean {
  const a = b.acceptance;
  return a.artifact.startsWith(b.owner_path) && a.then.includes(a.artifact);
}

function checkC3AccNontaut(b: ModuleBrief): boolean {
  const a = b.acceptance;
  if (a.then === a.given) return false;
  return !TAUTOLOGY_DENYLIST.some((re) => re.test(a.then));
}

function checkR17Deriv(b: ModuleBrief): boolean {
  if (b.guarantees.length < 1) return false;
  return b.guarantees.every((g) => {
    const d = g.derivation;
    if (d.kind === 'number') {
      return d.calc.trim() !== '' && /[0-9]/.test(d.calc) && CALC_ARITH.test(d.calc);
    }
    // structural
    return (
      d.enforced_by.trim() !== '' &&
      (ENFORCED_BY_PATTERN.test(d.enforced_by) ||
        /^type:/.test(d.enforced_by) ||
        /^gate:/.test(d.enforced_by))
    );
  });
}

function checkR19Absolute(b: ModuleBrief): boolean {
  return b.guarantees.every((g) => {
    if (!ABSOLUTE_CLAIM.test(g.claim)) return true;
    return g.label === 'kills_structural' && g.derivation.kind === 'structural';
  });
}

function checkR21Alts(b: ModuleBrief): boolean {
  // F02 §3.4/§10.1#4: the floor is flat 2, with NO grain='leaf' exemption. `grain` is a
  // proposer-authored field; letting it lower a depth floor was a one-word self-service exemption
  // from R21 (closed in the schema at the same time -- module-brief.v1.json's `alternatives` now
  // carries a flat `minItems: 2`, and lld-v1.ts's validator matches).
  if (b.alternatives.length < 2) return false;
  const alts: readonly KilledAlt[] = b.alternatives;
  const allFieldsPresent = alts.every(
    (a) => trimLen(a.option) !== 0 && trimLen(a.why_killed) !== 0 && trimLen(a.revive_trigger) !== 0,
  );
  if (!allFieldsPresent) return false;
  const validTriggers = alts.every((a) => {
    if (/^never$/i.test(a.revive_trigger.trim())) return true;
    return !NON_TRIGGER_DENYLIST.test(a.revive_trigger.trim());
  });
  if (!validTriggers) return false;
  const options = alts.map((a) => a.option.trim().toLowerCase());
  return new Set(options).size === options.length;
}

function checkR21Fail(b: ModuleBrief): boolean {
  const f = b.failure_story;
  return trimLen(f.trigger) >= 10 && trimLen(f.blast_radius) >= 10 && trimLen(f.fail_safe) >= 10;
}

function checkC12Store(b: ModuleBrief): boolean {
  if (!b.data_owned.every((d) => d.owned_by_node === b.node_id)) return false;
  const stores = b.data_owned.map((d) => d.store);
  return new Set(stores).size === stores.length;
}

function checkC12Deps(b: ModuleBrief, refs: GateRefs): boolean {
  if (b.deps.includes(b.node_id)) return false;
  if (!b.deps.every((d) => refs.known_node_ids.includes(d))) return false;
  const stores = new Set(b.data_owned.map((d) => d.store));
  return !b.deps.some((d) => stores.has(d));
}

function checkRegVerdict(b: ModuleBrief, refs: GateRefs): boolean {
  const r = b.registry;
  if (r.kind === 'install' || r.kind === 'extract') {
    return refs.registry_paths.includes(r.matched_path);
  }
  // build_new
  return r.searched.length >= 1 && r.searched.every((p) => refs.registry_paths.includes(p));
}

function checkIface(b: ModuleBrief): boolean {
  if (b.interface.length < 1) return false;
  return b.interface.every((decl) => {
    const s = decl.signature;
    return (s.includes('(') && s.includes(')')) || s.startsWith('type ') || s.startsWith('interface ');
  });
}

function checkShape(b: ModuleBrief): boolean {
  return (
    NODE_ID_PATTERN.test(b.node_id) &&
    b.purpose.length >= 1 &&
    b.purpose.length <= 200 &&
    b.owner_path.length > 0 &&
    !b.owner_path.includes('..')
  );
}

const CHECKS: ReadonlyArray<{
  readonly id: GateCheckId;
  readonly run: (b: ModuleBrief, refs: GateRefs) => boolean;
}> = [
  { id: 'C1-OPEN', run: (b) => checkC1Open(b) },
  { id: 'C2-OWNER', run: (b, refs) => checkC2Owner(b, refs) },
  { id: 'C3-ACC-PARSE', run: (b) => checkC3AccParse(b) },
  { id: 'C3-ACC-GROUND', run: (b) => checkC3AccGround(b) },
  { id: 'C3-ACC-NONTAUT', run: (b) => checkC3AccNontaut(b) },
  { id: 'R17-DERIV', run: (b) => checkR17Deriv(b) },
  { id: 'R19-ABSOLUTE', run: (b) => checkR19Absolute(b) },
  { id: 'R21-ALTS', run: (b) => checkR21Alts(b) },
  { id: 'R21-FAIL', run: (b) => checkR21Fail(b) },
  { id: 'C12-STORE', run: (b) => checkC12Store(b) },
  { id: 'C12-DEPS', run: (b, refs) => checkC12Deps(b, refs) },
  { id: 'REG-VERDICT', run: (b, refs) => checkRegVerdict(b, refs) },
  { id: 'IFACE', run: (b) => checkIface(b) },
  { id: 'SHAPE', run: (b) => checkShape(b) },
];

export function lldReady(brief: ModuleBrief, refs: GateRefs): GateVerdict {
  const checked = CHECKS.length;
  if (checked === 0) {
    return { outcome: 'MEASURED_NOTHING', checked: 0 };
  }

  const reasons: GateReason[] = [];
  for (const check of CHECKS) {
    let passed: boolean;
    try {
      passed = check.run(brief, refs);
    } catch {
      passed = false;
    }
    if (!passed) {
      reasons.push({ check_id: check.id, detail: `${check.id} failed` });
    }
  }

  const checksTotal = checked;
  const checksPassed = checksTotal - reasons.length;
  const ratio = Math.round((checksPassed / checksTotal) * 1000) / 1000;
  const score: DepthScore = {
    checks_total: checksTotal,
    checks_passed: checksPassed,
    ratio,
    failed_check_ids: reasons.map((r) => r.check_id),
  };

  if (reasons.length === 0) {
    return { outcome: 'READY', checked, score };
  }
  return { outcome: 'NOT_READY', checked, score, reasons };
}
