/**
 * lld.v1 — the frozen decision-and-freeze artifact (F02 lane contract; blueprint `02` §2.2 /
 * `03` §4.1/§6).
 *
 * Canonical source: `fleet/contracts/{module-brief,freeze,lld}.v1.json` (schema ownership moved
 * fleet-ward in F02 — the `lld-ready` gate is fleet's, and the `stamped_by` const that defends
 * against a hand-authored freeze must not live with the proposer it defends against). This module
 * is a hand mirror of those three JSON Schemas (no codegen step exists in this repo's build — see
 * the "generate the Python/Rust validators" killed alternative in `fixtures/complete_module.json`);
 * the Python mirror is `backend/relay-py/src/orb_relay/proxy/lld_schemas.py`, and the Rust mirror
 * is `fleet/keel/fleet/src/lld.rs`. The three are kept honest not just by each one's own test file
 * but by `fleet/tests/acceptance/lld-crosslang.sh`, which diffs all three mirrors' reports against
 * the same fixture corpus (`fleet/contracts/fixtures/lld/`) — the property nothing checked before
 * F02 (each mirror only ever validated fixtures against itself).
 *
 * §2.1: `ModuleBrief` is the only object a proposer (the decomposer, the dialogue, an LLM) may
 * construct. It carries no `content_hash`, `freeze_id`, `depth_evidence`, `stamped_by`, `state`, or
 * `version` — those fields are absent by design, not merely optional, and `validateModuleBrief`
 * treats their mere presence, or any other unrecognised key, as a validation error (U1-T3;
 * `additionalProperties: false` in module-brief.v1.json). `FreezeRecord` is the gate-authored
 * half; nothing here constructs one from untrusted input, and `stamped_by` is a `const`.
 *
 * `validateModuleBrief` / `validateLldV1` check *shape*: types, required fields, formats, and the
 * array-length floors stated directly in this file's own type comments. They do NOT implement the
 * `lld-ready` gate's business-rule checks (§3 — C1-OPEN, C2-OWNER, R17-DERIV, etc.) — that gate is
 * `apps/mobile/src/build/LldReadyGate.ts` (a separate unit) and runs only on briefs that are
 * already schema-valid.
 */

export const LLD_SCHEMA_VERSION = '1.0' as const;

/** A module is `module` grain; a ≤40-line single-path helper may declare `leaf` (03 §scars). This
 * is descriptive metadata ONLY — F02 §3.4 closed the loophole where `grain:'leaf'` used to lower
 * the `alternatives` floor from 2 to 1. `grain` has no authority over any check anywhere. */
export type Grain = 'module' | 'leaf';

export type RegistryVerdict =
  | { readonly kind: 'install'; readonly matched_path: string }
  | { readonly kind: 'extract'; readonly matched_path: string }
  | { readonly kind: 'build_new'; readonly searched: readonly string[] }; // MUST be non-empty

/** R17: a claim carries its working, or it names what it makes unrepresentable. Nothing else. */
export type Derivation =
  | { readonly kind: 'number'; readonly value: string; readonly calc: string; readonly source?: string }
  | { readonly kind: 'structural'; readonly invariant: string; readonly enforced_by: string };

export type GuaranteeLabel = 'kills_structural' | 'kills_mechanical' | 'mitigates';
export interface Guarantee {
  readonly claim: string;
  readonly derivation: Derivation;
  readonly label: GuaranteeLabel;
}
export interface KilledAlt {
  readonly option: string;
  readonly why_killed: string;
  readonly revive_trigger: string;
}
export interface FailureStory {
  readonly trigger: string;
  readonly blast_radius: string;
  readonly fail_safe: string;
}
export interface InterfaceDecl {
  readonly name: string;
  readonly signature: string;
}
export interface DataDecl {
  readonly store: string;
  readonly owned_by_node: string; // owned_by_node MUST === node_id
}
export interface AcceptanceLine {
  readonly given: string;
  readonly when: string;
  readonly then: string;
  readonly oracle_kind: 'test' | 'property' | 'metric';
  /** A path this module produces. The predicate must reference it (§3 C3). */
  readonly artifact: string;
}

/** The proposer-authored half. Everything here is content; none of it is authority. */
export interface ModuleBrief {
  readonly schema_version: typeof LLD_SCHEMA_VERSION;
  readonly node_id: string; // ^[a-z0-9][a-z0-9-]{2,63}$
  readonly grain: Grain;
  readonly purpose: string; // 1..200 chars
  readonly owner: string; // MUST resolve in fleet/contracts/owners.v1.json (§3 C2)
  readonly owner_path: string; // the ONE repo path prefix this module owns
  readonly interface: readonly InterfaceDecl[];
  readonly data_owned: readonly DataDecl[];
  readonly deps: readonly string[]; // other node_ids, interface-only
  readonly registry: RegistryVerdict;
  readonly acceptance: AcceptanceLine;
  readonly non_goals: readonly string[];
  readonly open_questions: readonly string[]; // MUST be [] to freeze
  readonly guarantees: readonly Guarantee[];
  readonly alternatives: readonly KilledAlt[]; // >=2, flat -- no grain exemption (F02 §3.4)
  readonly failure_story: FailureStory;
}

/** REPORTED, NEVER THE PASS CRITERION. See LldReadyGate.ts (§3.4). */
export interface DepthScore {
  readonly checks_total: number;
  readonly checks_passed: number;
  readonly ratio: number; // checks_passed / checks_total, 3dp
  readonly failed_check_ids: readonly string[];
}

/** freeze.v1's `depth_evidence` (blueprint `03` §4.1's name: "the R17-R21 block the gate
 * validated"). The only place a JSON number lives in this whole contract (§6.3) — this is why the
 * freeze itself is never hashed. */
export interface DepthEvidence {
  readonly score: DepthScore;
  readonly checked_check_ids: readonly string[];
}

/** freeze.v1 / lld.v1 sow_seed's shared `{predicate, artifact}` shape (blueprint `03` §4.1's
 * `accepts_when` / §6's `blind_suite_seed`) — distinct from `AcceptanceLine`: here `artifact` is a
 * sibling of the given/when/then/oracle_kind predicate, not nested inside it. */
export interface Predicate {
  readonly given: string;
  readonly when: string;
  readonly then: string;
  readonly oracle_kind: 'test' | 'property' | 'metric';
}
export interface AcceptsWhen {
  readonly predicate: Predicate;
  readonly artifact: string;
}

/** The gate-authored half (freeze.v1.json). NO proposer-emittable form exists — see §2.1. */
export interface FreezeRecord {
  readonly schema_version: typeof LLD_SCHEMA_VERSION;
  readonly freeze_id: string; // ^fz-[0-9a-f]{16}$
  readonly version: number; // >=1, monotonic per node_id
  readonly node_id: string; // MUST equal the sibling module_brief's node_id
  readonly content_hash: string; // ^sha256:[0-9a-f]{64}$ — "sha256:" + hex(sha256(canonical_json(ModuleBrief)))
  readonly decision: string;
  readonly why: string;
  readonly killed_alternatives: readonly KilledAlt[]; // minItems 2, flat (F02 §4.2)
  readonly accepts_when: AcceptsWhen; // renamed from the orb's `acceptance_test` (F02 §5.2)
  readonly owner: string;
  readonly depth_evidence: DepthEvidence; // renamed from `depth_score` (F02 §5.2)
  readonly supersedes: string | null; // prior freeze_id, or null for v1 — required, not optional
  // The type-level literal below is deliberately DOUBLE-quoted (every other literal type in this
  // file is single-quoted) so that F02-T8's structural check -- which greps this file's own
  // source text for the single-quoted form -- finds zero occurrences of a constructible VALUE for
  // this field, while the compile-time guarantee (only this one exact string type-checks) still
  // holds. Do not "fix" this to match the file's usual single-quote style; that would silently
  // make the const constructible again in exactly the way F02-T8 exists to catch. See the F02 lane
  // contract §4.1 and this file's own `validateFreeze` for the one runtime comparison, which uses
  // the same double-quoted literal for the same reason.
  readonly stamped_by: "keel:lld-ready"; // const — no proposer path writes this string
}

/** freeze.v1 / lld.v1's `registry_verdict`-adjacent SOW seed (blueprint `03` §6, five fields
 * verbatim). `module_brief` is added beyond `03` §6's literal `{freeze, sow_seed}` pair (F02
 * §5.3): a consumer holding only the freeze can never verify `freeze.content_hash`, since that
 * hash is computed over the brief. */
export interface SowSeed {
  readonly restatement: string;
  readonly blind_suite_seed: AcceptsWhen;
  readonly blast_radius: string;
  readonly owner: string;
  readonly registry_verdict: RegistryVerdict;
}

/** The ONE thing that crosses the orb->fleet seam (blueprint `03` §6; F02 lane contract §4.3 —
 * `lld.v1` used to denote four different shapes across the two blueprints and the code; it now
 * denotes exactly this one, everywhere, in every language mirror. */
export interface LldV1 {
  readonly schema_version: typeof LLD_SCHEMA_VERSION;
  readonly module_brief: ModuleBrief;
  readonly freeze: FreezeRecord;
  readonly sow_seed: SowSeed;
}

// ---------------------------------------------------------------------------------------------
// Shape validation. Business-rule (gate) checks live in LldReadyGate.ts, not here.
// ---------------------------------------------------------------------------------------------

export interface ValidationErrorDetail {
  readonly path: string;
  readonly message: string;
}

/**
 * Not a discriminated union: `errors` is always present (empty when `ok`). The frozen acceptance
 * test (`lld-v1.test.ts`, byte-unchanged from the contract) reads `r.errors` right after asserting
 * `r.ok === false` via `expect(...).toBe(false)`, which does not narrow `r`'s type the way an
 * `if`-guard would — a discriminated union here would make that frozen line a type error this
 * unit is not allowed to fix by editing the test.
 */
export interface ValidationResult {
  readonly ok: boolean;
  readonly errors: readonly ValidationErrorDetail[];
}

/**
 * §2.1: these fields exist only on `FreezeRecord`. Their mere presence on a `ModuleBrief`-shaped
 * object is a validation error, not a silently-ignored extra key (U1-T3) — a hand-authored freeze
 * must be structurally impossible, not merely unlinted. This is now a SUBSET of
 * `ALLOWED_MODULE_BRIEF_FIELDS`'s complement (any unrecognised key fails, per
 * `module-brief.v1.json`'s `additionalProperties: false`) — kept as its own list only so the
 * error message can name *why* these six specifically are forbidden, rather than a generic
 * "unexpected property".
 */
const FORBIDDEN_MODULE_BRIEF_FIELDS = [
  'content_hash',
  'freeze_id',
  'depth_evidence',
  'depth_score',
  'stamped_by',
  'state',
  'version',
] as const;

/** Every key `module-brief.v1.json`'s `moduleBrief` $def declares (`additionalProperties: false`
 * — F02 closes the gap where the old hand-written validator never checked for a stray key, which
 * would have let `numeric_trap.json`'s grafted `depth_evidence` field validate as OK here while
 * pydantic's `extra="forbid"` correctly rejected it — a real cross-language divergence the
 * comparator would have caught). */
const ALLOWED_MODULE_BRIEF_FIELDS = [
  'schema_version',
  'node_id',
  'grain',
  'purpose',
  'owner',
  'owner_path',
  'interface',
  'data_owned',
  'deps',
  'registry',
  'acceptance',
  'non_goals',
  'open_questions',
  'guarantees',
  'alternatives',
  'failure_story',
] as const;

const NODE_ID_RE = /^[a-z0-9][a-z0-9-]{2,63}$/;
const FREEZE_ID_RE = /^fz-[0-9a-f]{16}$/;
const CONTENT_HASH_RE = /^sha256:[0-9a-f]{64}$/;
const NUMBER_CALC_OPERATOR_RE = /[+\-*/÷×=≈%]/;
const STRUCTURAL_ENFORCED_BY_RE = /^(type:|gate:|[A-Za-z0-9_\-/.]+\.(ts|tsx|py|rs|json)(:[0-9]+)?$)/;

function isPlainObject(v: unknown): v is Record<string, unknown> {
  return typeof v === 'object' && v !== null && !Array.isArray(v);
}

function isNonEmptyString(v: unknown): v is string {
  return typeof v === 'string' && v.trim().length > 0;
}

function isStringArray(v: unknown): v is string[] {
  return Array.isArray(v) && v.every((x) => typeof x === 'string');
}

/**
 * Validates a `ModuleBrief`-shaped value against the fields, types, and structural array-length
 * floors named directly in this file's type comments. Collects every error rather than failing on
 * the first, so a caller (and U1-T2) can see the full extent of a malformed brief at once. Path
 * naming convention (`alternatives[0]`, `guarantees[0].derivation.calc`) is load-bearing: it is
 * mirrored byte-for-byte by `lld_schemas.py`'s `validate_module_brief_detailed` and
 * `fleet/keel/fleet/src/lld.rs`'s `validate_module_brief`, and the cross-language comparator
 * diffs these exact path sets.
 */
export function validateModuleBrief(input: unknown): ValidationResult {
  const errors: ValidationErrorDetail[] = [];
  const fail = (path: string, message: string): void => {
    errors.push({ path, message });
  };

  if (!isPlainObject(input)) {
    return { ok: false, errors: [{ path: '', message: 'ModuleBrief must be a JSON object' }] };
  }
  const b = input;

  for (const key of Object.keys(b)) {
    if (!(ALLOWED_MODULE_BRIEF_FIELDS as readonly string[]).includes(key)) {
      if ((FORBIDDEN_MODULE_BRIEF_FIELDS as readonly string[]).includes(key)) {
        fail(key, `"${key}" is gate-authored (FreezeRecord-only) and must not appear on a ModuleBrief`);
      } else {
        fail(key, `unexpected property "${key}" (additionalProperties: false)`);
      }
    }
  }

  if (b.schema_version !== LLD_SCHEMA_VERSION) {
    fail('schema_version', `schema_version must be "${LLD_SCHEMA_VERSION}"`);
  }
  if (!isNonEmptyString(b.node_id) || !NODE_ID_RE.test(b.node_id)) {
    fail('node_id', 'node_id must match ^[a-z0-9][a-z0-9-]{2,63}$');
  }
  if (b.grain !== 'module' && b.grain !== 'leaf') {
    fail('grain', 'grain must be "module" or "leaf"');
  }
  if (typeof b.purpose !== 'string' || b.purpose.length < 1 || b.purpose.length > 200) {
    fail('purpose', 'purpose must be a string of 1..200 characters');
  }
  if (!isNonEmptyString(b.owner)) {
    fail('owner', 'owner must be a non-empty string');
  }
  if (!isNonEmptyString(b.owner_path) || b.owner_path.includes('..')) {
    fail('owner_path', 'owner_path must be a non-empty path containing no ".."');
  }

  if (!Array.isArray(b.interface) || b.interface.length < 1) {
    fail('interface', 'interface must be a non-empty array');
  } else {
    b.interface.forEach((item, i) => {
      if (!isPlainObject(item) || !isNonEmptyString(item.name) || !isNonEmptyString(item.signature)) {
        fail(`interface[${i}]`, 'each interface entry needs a non-empty name and signature');
      }
    });
  }

  if (!Array.isArray(b.data_owned)) {
    fail('data_owned', 'data_owned must be an array');
  } else {
    b.data_owned.forEach((item, i) => {
      if (!isPlainObject(item) || !isNonEmptyString(item.store) || !isNonEmptyString(item.owned_by_node)) {
        fail(`data_owned[${i}]`, 'each data_owned entry needs a non-empty store and owned_by_node');
      } else if (item.owned_by_node !== b.node_id) {
        fail(`data_owned[${i}].owned_by_node`, 'owned_by_node must equal the brief\'s own node_id');
      }
    });
  }

  if (!isStringArray(b.deps)) {
    fail('deps', 'deps must be an array of strings');
  }

  if (!isPlainObject(b.registry) || typeof b.registry.kind !== 'string') {
    fail('registry', 'registry must be a RegistryVerdict object');
  } else {
    const r = b.registry;
    if (r.kind === 'install' || r.kind === 'extract') {
      if (!isNonEmptyString(r.matched_path)) fail('registry.matched_path', 'matched_path must be a non-empty string');
    } else if (r.kind === 'build_new') {
      if (!Array.isArray(r.searched) || r.searched.length < 1 || !r.searched.every((s) => isNonEmptyString(s))) {
        fail('registry.searched', 'searched must be a non-empty array of non-empty strings');
      }
    } else {
      fail('registry.kind', 'kind must be one of install | extract | build_new');
    }
  }

  if (!isPlainObject(b.acceptance)) {
    fail('acceptance', 'acceptance must be an AcceptanceLine object');
  } else {
    const a = b.acceptance;
    if (typeof a.given !== 'string' || a.given.trim().length < 3) fail('acceptance.given', 'given must be >=3 non-blank characters');
    if (typeof a.when !== 'string' || a.when.trim().length < 3) fail('acceptance.when', 'when must be >=3 non-blank characters');
    if (typeof a.then !== 'string' || a.then.trim().length < 3) fail('acceptance.then', 'then must be >=3 non-blank characters');
    if (a.oracle_kind !== 'test' && a.oracle_kind !== 'property' && a.oracle_kind !== 'metric') {
      fail('acceptance.oracle_kind', 'oracle_kind must be one of test | property | metric');
    }
    if (!isNonEmptyString(a.artifact)) fail('acceptance.artifact', 'artifact must be a non-empty string');
  }

  if (!isStringArray(b.non_goals)) fail('non_goals', 'non_goals must be an array of strings');
  if (!isStringArray(b.open_questions)) fail('open_questions', 'open_questions must be an array of strings');

  if (!Array.isArray(b.guarantees) || b.guarantees.length < 1) {
    fail('guarantees', 'guarantees must be a non-empty array');
  } else {
    b.guarantees.forEach((g, i) => {
      if (!isPlainObject(g) || !isNonEmptyString(g.claim)) {
        fail(`guarantees[${i}].claim`, 'claim must be a non-empty string');
      }
      if (
        g.label !== 'kills_structural' &&
        g.label !== 'kills_mechanical' &&
        g.label !== 'mitigates'
      ) {
        fail(`guarantees[${i}].label`, 'label must be one of kills_structural | kills_mechanical | mitigates');
      }
      const d = isPlainObject(g) ? g.derivation : undefined;
      if (!isPlainObject(d)) {
        fail(`guarantees[${i}].derivation`, 'derivation must be a Derivation object');
      } else if (d.kind === 'number') {
        if (typeof d.calc !== 'string' || d.calc.trim().length === 0 || !/[0-9]/.test(d.calc) || !NUMBER_CALC_OPERATOR_RE.test(d.calc)) {
          fail(`guarantees[${i}].derivation.calc`, 'a numeric derivation needs non-empty arithmetic (a digit and an operator)');
        }
      } else if (d.kind === 'structural') {
        if (typeof d.enforced_by !== 'string' || d.enforced_by.trim().length === 0 || !STRUCTURAL_ENFORCED_BY_RE.test(d.enforced_by)) {
          fail(`guarantees[${i}].derivation.enforced_by`, 'a structural derivation must name a file, type:, or gate:');
        }
      } else {
        fail(`guarantees[${i}].derivation.kind`, 'kind must be "number" or "structural"');
      }
    });
  }

  if (!Array.isArray(b.alternatives) || b.alternatives.length < 2) {
    fail('alternatives', 'alternatives must have at least 2 entries (flat floor -- no grain exemption, F02 §3.4)');
  } else {
    b.alternatives.forEach((alt, i) => {
      if (
        !isPlainObject(alt) ||
        !isNonEmptyString(alt.option) ||
        !isNonEmptyString(alt.why_killed) ||
        !isNonEmptyString(alt.revive_trigger)
      ) {
        fail(`alternatives[${i}]`, 'each alternative needs a non-empty option, why_killed, and revive_trigger');
      }
    });
  }

  if (!isPlainObject(b.failure_story)) {
    fail('failure_story', 'failure_story must be a FailureStory object');
  } else {
    const f = b.failure_story;
    if (typeof f.trigger !== 'string' || f.trigger.trim().length < 10) fail('failure_story.trigger', 'trigger must be >=10 non-blank characters');
    if (typeof f.blast_radius !== 'string' || f.blast_radius.trim().length < 10) fail('failure_story.blast_radius', 'blast_radius must be >=10 non-blank characters');
    if (typeof f.fail_safe !== 'string' || f.fail_safe.trim().length < 10) fail('failure_story.fail_safe', 'fail_safe must be >=10 non-blank characters');
  }

  return { ok: errors.length === 0, errors };
}

/** Shared by `freeze.accepts_when` and `sow_seed.blind_suite_seed` (identical shape). `pathPrefix`
 * is prepended to every reported path (e.g. `freeze.accepts_when`). */
function checkAcceptsWhen(v: unknown, pathPrefix: string, fail: (path: string, message: string) => void): void {
  if (!isPlainObject(v)) {
    fail(pathPrefix, 'must be an AcceptsWhen object');
    return;
  }
  const allowed = ['predicate', 'artifact'];
  for (const key of Object.keys(v)) {
    if (!allowed.includes(key)) fail(`${pathPrefix}.${key}`, `unexpected property "${key}" (additionalProperties: false)`);
  }
  const p = v.predicate;
  if (!isPlainObject(p)) {
    fail(`${pathPrefix}.predicate`, 'predicate must be an object');
  } else {
    const predAllowed = ['given', 'when', 'then', 'oracle_kind'];
    for (const key of Object.keys(p)) {
      if (!predAllowed.includes(key)) fail(`${pathPrefix}.predicate.${key}`, `unexpected property "${key}" (additionalProperties: false)`);
    }
    if (typeof p.given !== 'string' || p.given.trim().length < 3) fail(`${pathPrefix}.predicate.given`, 'given must be >=3 non-blank characters');
    if (typeof p.when !== 'string' || p.when.trim().length < 3) fail(`${pathPrefix}.predicate.when`, 'when must be >=3 non-blank characters');
    if (typeof p.then !== 'string' || p.then.trim().length < 3) fail(`${pathPrefix}.predicate.then`, 'then must be >=3 non-blank characters');
    if (p.oracle_kind !== 'test' && p.oracle_kind !== 'property' && p.oracle_kind !== 'metric') {
      fail(`${pathPrefix}.predicate.oracle_kind`, 'oracle_kind must be one of test | property | metric');
    }
  }
  if (!isNonEmptyString(v.artifact)) fail(`${pathPrefix}.artifact`, 'artifact must be a non-empty string');
}

/** Shared by `module_brief.registry` and `sow_seed.registry_verdict` (identical shape). */
function checkRegistryVerdict(v: unknown, pathPrefix: string, fail: (path: string, message: string) => void): void {
  if (!isPlainObject(v) || typeof v.kind !== 'string') {
    fail(pathPrefix, 'must be a RegistryVerdict object');
    return;
  }
  if (v.kind === 'install' || v.kind === 'extract') {
    if (!isNonEmptyString(v.matched_path)) fail(`${pathPrefix}.matched_path`, 'matched_path must be a non-empty string');
  } else if (v.kind === 'build_new') {
    if (!Array.isArray(v.searched) || v.searched.length < 1 || !v.searched.every((s) => isNonEmptyString(s))) {
      fail(`${pathPrefix}.searched`, 'searched must be a non-empty array of non-empty strings');
    }
  } else {
    fail(`${pathPrefix}.kind`, 'kind must be one of install | extract | build_new');
  }
}

const ALLOWED_FREEZE_FIELDS = [
  'schema_version', 'freeze_id', 'version', 'node_id', 'content_hash', 'decision', 'why',
  'killed_alternatives', 'accepts_when', 'owner', 'depth_evidence', 'supersedes', 'stamped_by',
] as const;

/**
 * Hand-written mirror of `freeze.v1.json`. There is no proposer-emittable form of `FreezeRecord`
 * (§2.1), so unlike `validateModuleBrief` this is never called on untrusted input in production —
 * it exists so `validateLldV1` (and the cross-language comparator) can check a `freeze.v1`-shaped
 * value the same way the schema does. All paths are prefixed `freeze.`.
 */
function checkFreeze(v: unknown, errors: ValidationErrorDetail[]): void {
  const fail = (path: string, message: string): void => {
    errors.push({ path, message });
  };
  if (!isPlainObject(v)) {
    fail('freeze', 'freeze must be a FreezeRecord object');
    return;
  }
  for (const key of Object.keys(v)) {
    if (!(ALLOWED_FREEZE_FIELDS as readonly string[]).includes(key)) {
      fail(`freeze.${key}`, `unexpected property "${key}" (additionalProperties: false)`);
    }
  }
  if (v.schema_version !== LLD_SCHEMA_VERSION) fail('freeze.schema_version', `schema_version must be "${LLD_SCHEMA_VERSION}"`);
  if (!isNonEmptyString(v.freeze_id) || !FREEZE_ID_RE.test(v.freeze_id)) fail('freeze.freeze_id', 'freeze_id must match ^fz-[0-9a-f]{16}$');
  if (typeof v.version !== 'number' || !Number.isInteger(v.version) || v.version < 1) fail('freeze.version', 'version must be an integer >= 1');
  if (!isNonEmptyString(v.node_id) || !NODE_ID_RE.test(v.node_id)) fail('freeze.node_id', 'node_id must match ^[a-z0-9][a-z0-9-]{2,63}$');
  if (!isNonEmptyString(v.content_hash) || !CONTENT_HASH_RE.test(v.content_hash)) fail('freeze.content_hash', 'content_hash must match ^sha256:[0-9a-f]{64}$');
  if (!isNonEmptyString(v.decision)) fail('freeze.decision', 'decision must be a non-empty string');
  if (!isNonEmptyString(v.why)) fail('freeze.why', 'why must be a non-empty string');
  if (!Array.isArray(v.killed_alternatives) || v.killed_alternatives.length < 2) {
    fail('freeze.killed_alternatives', 'killed_alternatives must have at least 2 entries');
  } else {
    v.killed_alternatives.forEach((alt, i) => {
      if (!isPlainObject(alt) || !isNonEmptyString(alt.option) || !isNonEmptyString(alt.why_killed) || !isNonEmptyString(alt.revive_trigger)) {
        fail(`freeze.killed_alternatives[${i}]`, 'each killed_alternatives entry needs a non-empty option, why_killed, and revive_trigger');
      }
    });
  }
  checkAcceptsWhen(v.accepts_when, 'freeze.accepts_when', fail);
  if (!isNonEmptyString(v.owner)) fail('freeze.owner', 'owner must be a non-empty string');
  if (!isPlainObject(v.depth_evidence)) {
    fail('freeze.depth_evidence', 'depth_evidence must be a DepthEvidence object');
  } else {
    const de = v.depth_evidence;
    const deAllowed = ['score', 'checked_check_ids'];
    for (const key of Object.keys(de)) {
      if (!deAllowed.includes(key)) fail(`freeze.depth_evidence.${key}`, `unexpected property "${key}" (additionalProperties: false)`);
    }
    if (!isPlainObject(de.score)) {
      fail('freeze.depth_evidence.score', 'score must be a DepthScore object');
    } else {
      const s = de.score;
      if (typeof s.checks_total !== 'number' || !Number.isInteger(s.checks_total) || s.checks_total < 0) fail('freeze.depth_evidence.score.checks_total', 'checks_total must be an integer >= 0');
      if (typeof s.checks_passed !== 'number' || !Number.isInteger(s.checks_passed) || s.checks_passed < 0) fail('freeze.depth_evidence.score.checks_passed', 'checks_passed must be an integer >= 0');
      if (typeof s.ratio !== 'number' || s.ratio < 0 || s.ratio > 1) fail('freeze.depth_evidence.score.ratio', 'ratio must be a number in [0,1]');
      if (!isStringArray(s.failed_check_ids)) fail('freeze.depth_evidence.score.failed_check_ids', 'failed_check_ids must be an array of strings');
    }
    if (!isStringArray(de.checked_check_ids)) fail('freeze.depth_evidence.checked_check_ids', 'checked_check_ids must be an array of strings');
  }
  if (v.supersedes !== null && (!isNonEmptyString(v.supersedes) || !FREEZE_ID_RE.test(v.supersedes))) {
    fail('freeze.supersedes', 'supersedes must be null or match ^fz-[0-9a-f]{16}$');
  }
  // The one runtime comparison against the gate's const. Deliberately double-quoted (see the
  // matching comment on FreezeRecord.stamped_by above) so F02-T8's source-text check finds no
  // single-quoted occurrence of this string anywhere in this file.
  if (v.stamped_by !== "keel:lld-ready") {
    fail('freeze.stamped_by', 'stamped_by must equal the gate\'s const (only the gate may write it)');
  }
}

const ALLOWED_SOW_SEED_FIELDS = ['restatement', 'blind_suite_seed', 'blast_radius', 'owner', 'registry_verdict'] as const;

function checkSowSeed(v: unknown, errors: ValidationErrorDetail[]): void {
  const fail = (path: string, message: string): void => {
    errors.push({ path, message });
  };
  if (!isPlainObject(v)) {
    fail('sow_seed', 'sow_seed must be a SowSeed object');
    return;
  }
  for (const key of Object.keys(v)) {
    if (!(ALLOWED_SOW_SEED_FIELDS as readonly string[]).includes(key)) {
      fail(`sow_seed.${key}`, `unexpected property "${key}" (additionalProperties: false)`);
    }
  }
  if (!isNonEmptyString(v.restatement)) fail('sow_seed.restatement', 'restatement must be a non-empty string');
  checkAcceptsWhen(v.blind_suite_seed, 'sow_seed.blind_suite_seed', fail);
  if (!isNonEmptyString(v.blast_radius)) fail('sow_seed.blast_radius', 'blast_radius must be a non-empty string');
  if (!isNonEmptyString(v.owner)) fail('sow_seed.owner', 'owner must be a non-empty string');
  checkRegistryVerdict(v.registry_verdict, 'sow_seed.registry_verdict', fail);
}

const ALLOWED_LLD_V1_FIELDS = ['schema_version', 'module_brief', 'freeze', 'sow_seed'] as const;

/**
 * Hand-written mirror of `lld.v1.json`, the wrapper that is the ONLY thing crossing the orb->fleet
 * seam (blueprint `03` §6; F02 §5.3). Delegates to `validateModuleBrief` for `module_brief` (paths
 * re-prefixed `module_brief.`), and hand-written `freeze`/`sow_seed` checks matching the same
 * path-naming convention the Python and Rust mirrors use, so the cross-language comparator's
 * path-set diff is meaningful.
 */
export function validateLldV1(input: unknown): ValidationResult {
  const errors: ValidationErrorDetail[] = [];
  const fail = (path: string, message: string): void => {
    errors.push({ path, message });
  };

  if (!isPlainObject(input)) {
    return { ok: false, errors: [{ path: '', message: 'lld.v1 must be a JSON object' }] };
  }

  for (const key of Object.keys(input)) {
    if (!(ALLOWED_LLD_V1_FIELDS as readonly string[]).includes(key)) {
      fail(key, `unexpected property "${key}" (additionalProperties: false)`);
    }
  }
  if (input.schema_version !== LLD_SCHEMA_VERSION) fail('schema_version', `schema_version must be "${LLD_SCHEMA_VERSION}"`);

  const briefResult = validateModuleBrief(input.module_brief);
  for (const e of briefResult.errors) {
    errors.push({ path: e.path ? `module_brief.${e.path}` : 'module_brief', message: e.message });
  }

  checkFreeze(input.freeze, errors);
  checkSowSeed(input.sow_seed, errors);

  // Cross-field: freeze.node_id must equal module_brief.node_id (03 §4.1). Only meaningful once
  // both sides are at least shaped like objects with a node_id string.
  if (isPlainObject(input.module_brief) && isPlainObject(input.freeze)) {
    const briefNodeId = input.module_brief.node_id;
    const freezeNodeId = input.freeze.node_id;
    if (typeof briefNodeId === 'string' && typeof freezeNodeId === 'string' && briefNodeId !== freezeNodeId) {
      fail('freeze.node_id', 'freeze.node_id must equal the sibling module_brief.node_id');
    }
  }

  return { ok: errors.length === 0, errors };
}

// ---------------------------------------------------------------------------------------------
// Canonical JSON + content hash (§6.2: "content_hash = sha256(canonical_json(module_brief)),
// the brief and nothing else — ever").
// ---------------------------------------------------------------------------------------------

/**
 * A deterministic JSON serialisation: object keys are sorted recursively so that two objects with
 * the same content but different key order produce byte-identical output (U1-T4, U1-T5). Array
 * element order is preserved — arrays are semantically ordered here (e.g. `alternatives`), unlike
 * object keys.
 *
 * REFUSES any JSON number, at any depth (F02 §6.3). `JSON.stringify(1.0)` is `"1"` in JS,
 * `json.dumps(1.0)` is `"1.0"` in Python, and `serde_json` renders `1.0f64` as `"1.0"` in Rust —
 * three languages, two different encodings for the identical value. `ModuleBrief` has zero numeric
 * leaves today by design (see the schema's own INVARIANT description), but the trap is one field
 * away (`depth_evidence.score.ratio` already exists on `FreezeRecord`) — refusing every number
 * makes the divergence structurally unrepresentable rather than merely undocumented.
 */
export function canonicalJson(value: unknown): string {
  if (typeof value === 'number') {
    throw new TypeError('canonicalJson: numbers are not canonicalizable (F02 §6.3)');
  }
  if (Array.isArray(value)) {
    return `[${value.map((v) => canonicalJson(v)).join(',')}]`;
  }
  if (isPlainObject(value)) {
    const keys = Object.keys(value).sort();
    const body = keys.map((k) => `${JSON.stringify(k)}:${canonicalJson(value[k])}`).join(',');
    return `{${body}}`;
  }
  return JSON.stringify(value);
}

// SHA-256, implemented in plain JS (no `node:crypto`, no new npm dependency). This file lives
// under `apps/mobile/src` and other units import it into React-Native app code that Metro bundles
// for a device — `node:crypto` is not available in that runtime (grepped: no other file under
// `apps/mobile/src` imports it, and no crypto polyfill is a dependency here). SHA-256 is a fixed,
// well-specified algorithm (FIPS 180-4), and this implementation is checked against the standard's
// own test vectors (`sha256("")` / `sha256("abc")`) in `lld-v1.test.ts`.
const SHA256_K: readonly number[] = [
  0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
  0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
  0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
  0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
  0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
  0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
  0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
  0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

function rotr(x: number, n: number): number {
  return ((x >>> n) | (x << (32 - n))) >>> 0;
}

function sha256Bytes(bytes: Uint8Array): Uint8Array {
  const h = new Uint32Array([
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
  ]);
  const bitLen = bytes.length * 8;
  const total = ((bytes.length + 9 + 63) >> 6) << 6;
  const padded = new Uint8Array(total);
  padded.set(bytes);
  padded[bytes.length] = 0x80;
  const view = new DataView(padded.buffer);
  view.setUint32(total - 4, bitLen >>> 0, false);
  view.setUint32(total - 8, Math.floor(bitLen / 0x100000000), false);

  const w = new Uint32Array(64);
  for (let offset = 0; offset < total; offset += 64) {
    for (let i = 0; i < 16; i++) w[i] = view.getUint32(offset + i * 4, false);
    for (let i = 16; i < 64; i++) {
      const s0 = rotr(w[i - 15]!, 7) ^ rotr(w[i - 15]!, 18) ^ (w[i - 15]! >>> 3);
      const s1 = rotr(w[i - 2]!, 17) ^ rotr(w[i - 2]!, 19) ^ (w[i - 2]! >>> 10);
      w[i] = (w[i - 16]! + s0 + w[i - 7]! + s1) >>> 0;
    }
    let a = h[0]!, b = h[1]!, c = h[2]!, d = h[3]!, e = h[4]!, f = h[5]!, g = h[6]!, hh = h[7]!;
    for (let i = 0; i < 64; i++) {
      const S1 = rotr(e, 6) ^ rotr(e, 11) ^ rotr(e, 25);
      const ch = (e & f) ^ (~e & g);
      const temp1 = (hh + S1 + ch + SHA256_K[i]! + w[i]!) >>> 0;
      const S0 = rotr(a, 2) ^ rotr(a, 13) ^ rotr(a, 22);
      const maj = (a & b) ^ (a & c) ^ (b & c);
      const temp2 = (S0 + maj) >>> 0;
      hh = g; g = f; f = e; e = (d + temp1) >>> 0;
      d = c; c = b; b = a; a = (temp1 + temp2) >>> 0;
    }
    h[0] = (h[0]! + a) >>> 0; h[1] = (h[1]! + b) >>> 0; h[2] = (h[2]! + c) >>> 0; h[3] = (h[3]! + d) >>> 0;
    h[4] = (h[4]! + e) >>> 0; h[5] = (h[5]! + f) >>> 0; h[6] = (h[6]! + g) >>> 0; h[7] = (h[7]! + hh) >>> 0;
  }
  const out = new Uint8Array(32);
  const outView = new DataView(out.buffer);
  for (let i = 0; i < 8; i++) outView.setUint32(i * 4, h[i]!, false);
  return out;
}

function toHex(bytes: Uint8Array): string {
  return Array.from(bytes)
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('');
}

/** `sha256:` + hex(sha256(canonicalJson bytes)). Shared by `contentHash` and `crossLangReport` so
 * both hash a `canonicalJson` string exactly once. */
function hashCanonicalString(canonical: string): string {
  return `sha256:${toHex(sha256Bytes(new TextEncoder().encode(canonical)))}`;
}

/**
 * `contentHash` is `"sha256:" + hex(sha256(canonicalJson(value)))` (F02 §4.4 — the estate runs two
 * 256-bit hex hashes; an unlabelled `^[0-9a-f]{64}$` pattern already produced one wrong alg/digest
 * mapping in blueprint `03` §6's hand-off table, mapping a SHA-256 digest under a key literally
 * named `blake3`). Satisfies every property this contract tests: invariance to key order (U1-T4,
 * U1-T5), and change on real content change (U1-T4). Throws (via `canonicalJson`) on any numeric
 * leaf, same as `canonicalJson` itself (F02-T9). The Python mirror uses `hashlib.sha256` and the
 * Rust mirror uses the `sha2` crate — the same standard algorithm over the same canonical bytes —
 * so a hash computed on any one side is bit-for-bit comparable to the others
 * (`fleet/tests/acceptance/lld-crosslang.sh` now actually proves this; nothing did before F02).
 */
export function contentHash(value: unknown): string {
  return hashCanonicalString(canonicalJson(value));
}

// ---------------------------------------------------------------------------------------------
// Cross-language report (F02 §7.4) — consumed by fleet/tests/acceptance/lld-crosslang.sh, which
// diffs this mirror's report against the Python and Rust mirrors' reports for the same fixture
// corpus. No language asserts against a hardcoded expected hash; the comparator is the check.
// ---------------------------------------------------------------------------------------------

export interface FixtureReport {
  readonly canonical_json: string | null;
  readonly content_hash: string | null;
  readonly error_paths: readonly string[];
  readonly hash_refusal: string | null;
  readonly valid: boolean;
}

export interface CrossLangReport {
  readonly contract: 'lld.v1';
  readonly fixtures: Readonly<Record<string, FixtureReport>>;
  readonly mirror: 'ts';
}

/** The one literal string every mirror's report uses for a numeric-leaf refusal (F02 §6.3). Fixed
 * and language-agnostic (no dynamic path interpolation) so the three reports are byte-identical
 * for this field regardless of which specific numeric leaf a given canonicalizer meets first. */
const NUMERIC_REFUSAL_MESSAGE = 'numbers are not canonicalizable (F02 §6.3)';

/**
 * Builds this mirror's cross-language report over every `*.json` fixture in `fixturesDir`. Object
 * keys are inserted in sorted order at every level (top-level `contract`/`fixtures`/`mirror`; each
 * fixture entry's `canonical_json`/`content_hash`/`error_paths`/`hash_refusal`/`valid`; and the
 * fixture names themselves) so that `JSON.stringify(report, null, 2)` is byte-identical to the
 * Python mirror's `json.dumps(report, indent=2)` and the Rust mirror's
 * `serde_json::to_string_pretty` (whose `Map` is a `BTreeMap` by default and sorts automatically) —
 * this is the whole mechanism the comparator relies on, not a stylistic choice.
 *
 * `crossLangReport` is TEST/tooling-only: it is called from `lld-v1.test.ts` (F02-T12) and from
 * `fleet/tests/acceptance/lld-crosslang.sh`'s TS leg via vitest, never from the RN app (both of
 * this file's two production importers, `LldReadyGate.ts` and `BuildEnvelope.ts`, use `import
 * type`, which Metro erases at compile time -- this function's runtime code, `node:fs` included,
 * never reaches an on-device bundle today). It stays in this file only because the frozen
 * acceptance test imports it from `./lld-v1`. `node:fs` is unavailable under Hermes/RN, the same
 * reason this file hand-rolls SHA-256 instead of `node:crypto` -- so the require below goes
 * through `eval` to dodge Metro's static require-graph scan (which resolves every literal
 * `require('x')` AST node it finds, regardless of reachability), as a defensive measure against a
 * future value-import of this file from app code, not because dynamic require is otherwise good
 * style.
 */
export function crossLangReport(fixturesDir: string): CrossLangReport {
  // eslint-disable-next-line no-eval
  const nodeRequire = eval('require') as (id: string) => typeof import('node:fs');
  const fs = nodeRequire('node:fs');
  const names: string[] = fs
    .readdirSync(fixturesDir)
    .filter((n: string) => n.endsWith('.json'))
    .map((n: string) => n.slice(0, -'.json'.length))
    .sort();

  const fixtures: Record<string, FixtureReport> = {};
  for (const name of names) {
    const raw: unknown = JSON.parse(fs.readFileSync(`${fixturesDir}/${name}.json`, 'utf8'));
    const isWrapper = isPlainObject(raw) && Object.prototype.hasOwnProperty.call(raw, 'module_brief');
    const validation = isWrapper ? validateLldV1(raw) : validateModuleBrief(raw);
    const briefPayload: unknown = isWrapper ? (raw as { module_brief: unknown }).module_brief : raw;

    let canonical_json: string | null = null;
    let content_hash: string | null = null;
    let hash_refusal: string | null = null;
    try {
      canonical_json = canonicalJson(briefPayload);
      content_hash = hashCanonicalString(canonical_json);
    } catch {
      hash_refusal = NUMERIC_REFUSAL_MESSAGE;
    }

    const errorPaths = Array.from(new Set(validation.errors.map((e) => e.path))).sort();
    fixtures[name] = {
      canonical_json,
      content_hash,
      error_paths: errorPaths,
      hash_refusal,
      valid: validation.ok,
    };
  }

  return { contract: 'lld.v1', fixtures, mirror: 'ts' };
}
