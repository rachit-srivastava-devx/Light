/**
 * lld.v1 — the frozen decision-and-freeze artifact (Speed-of-Thought P0 contract §2,
 * `docs/SPEED-OF-THOUGHT-P0-CONTRACT.md`).
 *
 * Canonical source: `contracts/lld.v1.json`. This module is a hand mirror of that JSON Schema
 * (no codegen step exists in this repo's build — see the "generate the Python validator" killed
 * alternative in `fixtures/complete_module.json`); the Python mirror is
 * `backend/relay-py/src/orb_relay/proxy/lld_schemas.py`. The two are kept honest by
 * `lld-v1.test.ts` (this unit) and `tests/test_lld_schema_conformance.py`, which run both
 * validators against the same two fixtures on every change.
 *
 * §2.1: `ModuleBrief` is the only object a proposer (the decomposer, the dialogue, an LLM) may
 * construct. It carries no `content_hash`, `freeze_id`, `depth_score`, `stamped_by`, `state`, or
 * `version` — those fields are absent by design, not merely optional, and `validateModuleBrief`
 * treats their mere presence as a validation error (U1-T3). `FreezeRecord` is the gate-authored
 * half; nothing here constructs one from untrusted input, and `stamped_by` is a `const`.
 *
 * `validateModuleBrief` checks *shape*: types, required fields, formats, and the array-length
 * floors that are stated directly in this file's own type comments (`alternatives` ≥2/≥1 by
 * grain). It does NOT implement the `lld-ready` gate's business-rule checks (§3 — C1-OPEN,
 * C2-OWNER, R17-DERIV, etc.) — that gate is `apps/mobile/src/build/LldReadyGate.ts` (a separate
 * unit, U3) and runs only on briefs that are already schema-valid.
 */

export const LLD_SCHEMA_VERSION = '1.0' as const;

/** A module is `module` grain; a ≤40-line single-path helper may declare `leaf` (03 §scars). */
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
  readonly node_id: string; // ^[a-z0-9][a-z0-9-]{2,63}$
  readonly grain: Grain;
  readonly purpose: string; // 1..200 chars
  readonly owner: string; // MUST resolve in contracts/owners.v1.json (§3 C2)
  readonly owner_path: string; // the ONE repo path prefix this module owns
  readonly interface: readonly InterfaceDecl[];
  readonly data_owned: readonly DataDecl[];
  readonly deps: readonly string[]; // other node_ids, interface-only
  readonly registry: RegistryVerdict;
  readonly acceptance: AcceptanceLine;
  readonly non_goals: readonly string[];
  readonly open_questions: readonly string[]; // MUST be [] to freeze
  readonly guarantees: readonly Guarantee[];
  readonly alternatives: readonly KilledAlt[]; // >=2 at module grain, >=1 at leaf
  readonly failure_story: FailureStory;
}

/** REPORTED, NEVER THE PASS CRITERION. See LldReadyGate.ts (§3.4). */
export interface DepthScore {
  readonly checks_total: number;
  readonly checks_passed: number;
  readonly ratio: number; // checks_passed / checks_total, 3dp
  readonly failed_check_ids: readonly string[];
}

/** The gate-authored half. NO proposer-emittable form exists — see §2.1. */
export interface FreezeRecord {
  readonly schema_version: typeof LLD_SCHEMA_VERSION;
  readonly freeze_id: string; // ^fz-[0-9a-f]{16}$
  readonly version: number; // >=1, monotonic per node_id
  readonly node_id: string;
  readonly content_hash: string; // ^[0-9a-f]{64}$ — hash(canonical_json(ModuleBrief))
  readonly decision: string; // == brief.purpose, canonicalised
  readonly why: string;
  readonly killed_alternatives: readonly KilledAlt[]; // minItems 2 (module) / 1 (leaf)
  readonly acceptance_test: AcceptanceLine;
  readonly owner: string;
  readonly depth_score: DepthScore;
  readonly supersedes: string | null; // prior freeze_id, or null for v1
  readonly stamped_by: 'orb:lld-ready'; // const — no proposer path writes this string
}

export type LldV1 = { readonly brief: ModuleBrief; readonly freeze: FreezeRecord };

// ---------------------------------------------------------------------------------------------
// Shape validation (U1). Business-rule (gate) checks live in LldReadyGate.ts (U3), not here.
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
 * must be structurally impossible, not merely unlinted.
 */
const FORBIDDEN_MODULE_BRIEF_FIELDS = [
  'content_hash',
  'freeze_id',
  'depth_score',
  'stamped_by',
  'state',
  'version',
] as const;

const NODE_ID_RE = /^[a-z0-9][a-z0-9-]{2,63}$/;
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
 * the first, so a caller (and U1-T2) can see the full extent of a malformed brief at once.
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

  for (const forbidden of FORBIDDEN_MODULE_BRIEF_FIELDS) {
    if (Object.prototype.hasOwnProperty.call(b, forbidden)) {
      fail(forbidden, `"${forbidden}" is gate-authored (FreezeRecord-only) and must not appear on a ModuleBrief`);
    }
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

  const altFloor = b.grain === 'leaf' ? 1 : 2;
  if (!Array.isArray(b.alternatives) || b.alternatives.length < altFloor) {
    fail('alternatives', `alternatives must have at least ${altFloor} entries at grain="${String(b.grain)}"`);
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

// ---------------------------------------------------------------------------------------------
// Canonical JSON + content hash (§2.1: "freeze_bytes = canonical_json(brief) only").
// ---------------------------------------------------------------------------------------------

/**
 * A deterministic JSON serialisation: object keys are sorted recursively so that two objects with
 * the same content but different key order produce byte-identical output (U1-T4, U1-T5). Array
 * element order is preserved — arrays are semantically ordered here (e.g. `alternatives`), unlike
 * object keys.
 */
export function canonicalJson(value: unknown): string {
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
// under `apps/mobile/src` and other units (U2/U3/U5) import it into React-Native app code that
// Metro bundles for a device — `node:crypto` is not available in that runtime (grepped: no other
// file under `apps/mobile/src` imports it, and no crypto polyfill is a dependency here). The
// contract's illustrative text names blake3; this repo has no blake3 dependency anywhere, and
// adding one for a schema unit is out of scope (see the "adopt ajv" killed alternative in
// `fixtures/complete_module.json` for the same reasoning applied to a different dependency).
// SHA-256 is a fixed, well-specified algorithm (FIPS 180-4), and this implementation is checked
// against the standard's own test vectors (`sha256("")` / `sha256("abc")`) in `lld-v1.test.ts`.
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

/**
 * `contentHash` is SHA-256 of `canonicalJson(value)`, hex-encoded (64 chars, matches
 * `FreezeRecord.content_hash`'s `^[0-9a-f]{64}$`). Satisfies every property this contract tests:
 * fixed 64-hex-char output, invariance to key order (U1-T4, U1-T5), and change on real content
 * change (U1-T4). The Python mirror (`lld_schemas.py`) uses `hashlib.sha256` — the same standard
 * algorithm over the same canonical bytes — so a hash computed on one side is bit-for-bit
 * comparable to the other, even though no frozen test currently asserts that cross-language
 * equality.
 */
export function contentHash(value: unknown): string {
  return toHex(sha256Bytes(new TextEncoder().encode(canonicalJson(value))));
}
