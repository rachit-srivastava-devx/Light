# F02 — `lld.v1` contract (lane contract)

| | |
|---|---|
| **Feature** | F02 — `lld.v1` contract: schema + TS mirror (orb) + Rust mirror (fleet) + Python mirror (orb), one shared fixture corpus |
| **Branch / worktree** | `lane/F02-lld-v1-contract` → `Light/.worktrees/F02-lld-v1-contract` |
| **Repos touched** | `fleet/` (schema owner) + `orb/` (consumer) |
| **Depends on** | S0 only |
| **Blocks** | F03, F05, F06, F07, F09, F35 — every one of them reads this shape |
| **Lead** | lead-architect (F02). Contract only; no production code written by the lead (T1). |
| **Merge class** | **Contract → human-merge always (A15 / D4).** Ship to a PR and stop. |

Written against `blueprints/Speed-of-Thought-L8-Deep-Dive/02-DIALOGUE-PLANE-BUILD-MODE.md` §2.2 and
`03-SPEC-COMPLETE-GATE-AND-DESIGN-GRAPH.md` §4.1 / §6, both read in full, plus the live tree.

---

## 1. Restatement — what is actually being asked, in this tree's own terms

FEATURES.md F02 asks for one versioned artifact that both engines agree on, mirrored into three
languages, proven equivalent by a shared fixture corpus.

That is not what exists. What exists is **a good three-quarters of it, in the wrong repo, under a
name that means three different things, with the one field that makes the whole design safe pointing
at the wrong stamper.** The real work of this lane is a *reconciliation*, not a greenfield build:

1. Move schema ownership from `orb/contracts/` to `fleet/contracts/` (FEATURES.md: "fleet (schema owner)").
2. Split the one overloaded `lld.v1` name into the three shapes the blueprint actually describes.
3. Fix four correctness defects the copy carried across (§4).
4. Add the missing Rust mirror.
5. Build the thing that does not exist anywhere today and is the entire point of the acceptance line:
   **a cross-language comparator that diffs what three runtimes actually emit.** Today each mirror
   validates the fixtures *by itself*. Nothing has ever compared TS's output to Python's.

The acceptance line as written in FEATURES.md — *"Same fixture round-trips through all 3 language
mirrors byte-identical"* — is not literally testable (a validator does not re-emit its input). §7
restates it as three things that are.

---

## 2. C1 / L2 registry verdict, said out loud

FEATURES.md F02 says **`build-new`. That is wrong, and this lane does not follow it.**

The verdict is **EXTRACT (C2) for the schema and two of the three mirrors; BUILD-NEW for the rest.**

### 2.1 What already exists (do not rewrite it)

Confirmed present in this worktree — the `2026-09-02-2200 — U1 lld-v1-schema` FLEET-LEARNINGS entry's
artifacts did come across in the D15 copy, plus more:

| Path | Lines | What it is | Verdict |
|---|---|---|---|
| `orb/contracts/lld.v1.json` | 195 | Full JSON Schema, draft-07, `additionalProperties:false` throughout, `$defs` for every sub-shape | **extract + adapt** |
| `orb/contracts/owners.v1.json` | 6 | Owner allow-list backing `C2-OWNER` | **extract as-is** |
| `orb/apps/mobile/src/build/lld-v1.ts` | 428 | TS mirror: types, `validateModuleBrief`, `canonicalJson`, hand-rolled SHA-256 checked against FIPS 180-4 vectors | **extract + adapt in place** |
| `orb/backend/relay-py/src/orb_relay/proxy/lld_schemas.py` | 241 | pydantic mirror, `ConfigDict(extra="forbid")` | **extract + adapt in place** |
| `orb/apps/mobile/src/build/lld-v1.test.ts` | 47 | Frozen acceptance U1-T1..U1-T5 | **extract; extend, do not weaken** |
| `orb/apps/mobile/src/build/fixtures/{complete_module,one_line_freeze}.json` | 2 files | The good/bad pair, with `AUTHORSHIP.md` recording independent authorship | **extract as the corpus seed** |
| `orb/backend/relay-py/tests/test_lld_schema_conformance.py` | — | Python-side fixture test | **extract + extend** |

Rewriting a hand-implemented SHA-256 that is already checked against the standard's own test vectors,
because a features table guessed "build-new", would be a C1 violation and a waste of a session.

### 2.2 What genuinely does not exist (build-new)

| Thing | Evidence it is absent |
|---|---|
| Any Rust `lld`/`ModuleBrief`/`freeze` type | `grep -rn "lld"` across `fleet/keel/` → 0 source hits (only PDF filenames under `fleet/output/`, `fleet/tmp/`) |
| The `03` §6 hand-off wrapper `LldReady{schema_version, freeze, sow_seed}` | 0 hits in either repo, in any language |
| A cross-language comparator | 0 hits. TS and Python each validate the same fixtures independently; nothing diffs their outputs |
| `sha2` in the fleet Rust crate | `fleet/keel/fleet/Cargo.toml` has `blake3 = "1"`, no `sha2` |

### 2.3 The precedent this lane must match

`fleet/crew/crew/schema.py:1-9` is *already* "pydantic models hand-mirrored from
`../contracts/*.v1.json`". That is the fleet-side pattern for exactly this problem, and it is in the
required gate (`fleet/verify.sh:98` — `cd crew && uv run pytest -q`). Match it.

It also supplies this lane's single best piece of evidence — see §4.5.

---

## 3. Killed alternatives

### 3.1 KILLED — keep `orb/contracts/lld.v1.json` as the canonical schema; fleet just reads it

The path of least resistance: leave the file where the copy put it, point the new Rust mirror at
`../../orb/contracts/lld.v1.json`, done in an hour.

**Why killed.** The `lld-ready` gate is **fleet's** (F06: "deterministic (0 LLM), Rust"). `03` §4.1
pins `stamped_by` to `const "keel:lld-ready"` precisely so that *no proposer path can write it*. If
the schema lives in the orb — the proposer — then the orb owns the definition of the string that
exists to prove the orb did not author the freeze. The const stops being a structural guarantee and
becomes an honour system. Ownership has to sit with the refuser, not the proposer. Additionally,
`fleet/contracts/` is already the estate's contract directory (4 schemas, one gate, one style); a
fifth contract living somewhere else guarantees the style diverges.

**Revive trigger.** If fleet is ever removed from the loop and the orb both proposes and freezes
(i.e. F06 moves back into the orb), schema ownership follows the gate back.

### 3.2 KILLED — three independently-maintained type definitions, no shared schema

"JSON Schema is ceremony; just write the TS interface, the pydantic model, and the Rust struct, and
keep them in sync in review."

**Why killed — this is not a hypothetical, it is happening in this repo right now.**

```
fleet/contracts/receipt.v1.json:15   "event": {"enum": ["run_start","artifact_frozen","attested",
                                                        "refusal","gate_verdict","run_end","lane_status"]}
fleet/crew/crew/schema.py:87-94      event: Literal["run_start","artifact_frozen","attested",
                                                     "refusal","gate_verdict","run_end"]      # ← no lane_status
```

Seven values in the schema, six in the mirror. fleet's Python mirror **rejects a receipt that fleet's
own Rust writes and fleet's own schema declares valid.** There is no test binding the two: the only
references to `contracts/` anywhere under `fleet/crew/` are a docstring (`schema.py:5-7`) and a
comment in a test (`tests/test_sow.py:14`) that literally says *"contract drift can break generated
consumers"*. Review-based sync produced a live drift in a two-mirror system maintained by careful
people. F02 adds a **fourth** runtime. Review does not scale to four.

**Revive trigger.** Never at this mirror count. If the system ever collapses to one runtime, the
schema file is redundant.

### 3.3 KILLED — generate the mirrors from the schema (codegen: `quicktype` / `datamodel-code-generator` / `typify`)

The obviously-correct answer to §3.2, and the one F35 (`Contract-codegen pipeline`) is eventually for.

**Why killed *for F02*.** Four reasons, in order of weight:

1. **The consumer runtimes cannot take the output.** The TS mirror is bundled by Metro into RN app
   code. Every JSON-Schema→TS generator emits types plus an `ajv` runtime; `ajv` compiles validators
   with `new Function`, which is unavailable under Hermes/RN. The existing mirror is hand-written
   *for that reason* and hand-rolls SHA-256 *for the same reason* (`lld-v1.ts:346-354`: no
   `node:crypto` in the Metro bundle).
2. **The mirrors are not pure shape.** `data_owned[].owned_by_node === node_id` (`lld-v1.ts:224`),
   `acceptance.artifact` grounding, the `Derivation` arithmetic check (`lld-v1.ts:283`) are all
   cross-field predicates no JSON Schema expresses and no generator emits.
3. **Adding a codegen step is a new required build stage in two repos** (`orb`'s `npm run verify`,
   `fleet/verify.sh`) plus a staleness gate. That is F35's whole scope, and F35 lists **F02 as its
   dependency** — F02 cannot depend on it back.
4. **A generator does not remove the drift risk, it relocates it.** You still need the
   cross-language fixture comparator (§7.4) to prove the generated code agrees. Build the comparator
   first; it is the part that has value with or without codegen.

**Revive trigger.** F35. When it lands, the comparator built here becomes its staleness gate — do not
throw it away.

### 3.4 KILLED — keep `grain: 'leaf'` lowering the `alternatives` floor from 2 to 1

`orb/contracts/lld.v1.json:150-154` and `lld-v1.ts:296` (`const altFloor = b.grain === 'leaf' ? 1 : 2`)
let a brief declare `grain: "leaf"` and thereby need only one killed alternative. **The same rule is
written a second time at `LldReadyGate.ts:134-135`** — so it must be deleted in three places, or the
schema fix is cosmetic and the gate still waves the brief through (§10.1 #4).

**Why killed.** `grain` is a **proposer-authored field** (it is in `moduleBrief.required`,
`lld.v1.json:132`). `03` §4.1 puts `minItems: 2` on `rejected` flat, and says why: *"the R21 floor is
in the schema, not a lint."* A proposer-declared field that lowers a depth floor is a one-word
self-service exemption from R21 — the estate's own "a check cheaper to fake than to satisfy will be
faked" principle, in its purest form. The floor goes to `2`, flat, in the schema and all three
mirrors. `grain` survives as descriptive metadata with no authority.

**Revive trigger.** If F06's calibration corpus shows ≥30% of genuine leaf-grain briefs have only one
credible alternative, reintroduce the exemption as a **gate-stamped** field derived from measured
size — never as a proposer-declared one.

### 3.5 KILLED — hand-implement SHA-256 in Rust the way the TS mirror does

Symmetry argues for it: three hand-rolled implementations, one shared test-vector suite, zero new deps.

**Why killed.** The TS mirror hand-rolls *under duress* — Metro/Hermes has no `node:crypto` and the
comment at `lld-v1.ts:346-354` says exactly that. Rust is under no such constraint. `sha2` (RustCrypto)
is MIT/Apache-2.0, which is on `fleet/keel/deny.toml`'s allow-list, so it clears `cargo-deny`
(`verify.sh:51`) with no exception entry. Hand-writing a crypto primitive where a vetted, ubiquitous,
license-clean crate exists is the estate's "solved problem — adopt, do not author" lesson inverted.

**Revive trigger.** If `cargo-deny` or `cargo-audit` ever flags `sha2`, or fleet gains a
no-std/wasm target that cannot take it.

### 3.6 KILLED — define a canonical number encoding (RFC 8785 JCS) so numeric fields can be hashed

The general fix for the FLEET-LEARNINGS numeric-encoding trap: implement JSON Canonicalization Scheme
number serialization in all three mirrors, then any field may be hashed.

**Why killed.** JCS numbers are the ECMA-262 shortest-round-trip algorithm. Getting it right in three
languages — `1.0`, `1e21`, `-0`, `5e-324`, `1e-7` — is a known-hard, high-consequence job, and this
contract needs **zero** numeric fields inside the hashed set (§6.3). Refusing to encode a number is
both cheaper and structurally stronger than encoding it correctly: if the canonicalizer *cannot*
serialize a number, no encoding divergence is representable.

**Revive trigger.** The day a numeric field must enter `content_hash`'s input. Then implement RFC 8785
number serialization in all three mirrors, with a cross-language fixture covering
`1.0 / 1e21 / -0 / 5e-324 / 1e-7`, and delete the refusal — do not hand-roll a "good enough" float
formatter.

---

## 4. The four defects the copy carried across

Each is a real behavioural difference from `02`/`03`, not a style nit. The builder fixes all four.

### 4.1 `stamped_by` names the wrong stamper — **the security-shaped one**

```
orb/contracts/lld.v1.json:190      "stamped_by": { "const": "orb:lld-ready" }
orb/apps/mobile/src/build/lld-v1.ts:114   readonly stamped_by: 'orb:lld-ready';
03 §4.1                            "stamped_by":   {"const":"keel:lld-ready"}
```

`03` §4.1's own justification: *"the one field a proposer would love to forge — `stamped_by` — pinned
to a `const` only the gate can satisfy (there is no proposer path that writes `keel:lld-ready`).
`kills (structural)` a hand-authored freeze."*

In Light the gate is fleet's (F06). `orb:lld-ready` is a string the orb — the proposer — can write.
The const that exists to make a hand-authored freeze impossible currently names the very actor it is
defending against. **Fix: `keel:lld-ready`.** The orb's mirrors must be able to *validate* it and must
have no code path that *constructs* it.

### 4.2 The R21 floor is missing from the freeze entirely

```
orb/contracts/lld.v1.json:185   "killed_alternatives": { "type": "array", "items": {...} }   ← no minItems
03 §4.1                         "rejected": {"type":"array","minItems":2, ...}
FEATURES.md F02                 "killed_alternatives[] ≥2"
```

The brief-side floor at least exists in the validators (`lld-v1.ts:296`, cross-field with `grain`).
The **freeze-side floor does not exist anywhere** — not in the schema, not in either mirror. A gate
could stamp a freeze with zero killed alternatives and every current check would pass. Fix: `minItems: 2`
on both `module_brief.alternatives` and `freeze.killed_alternatives`, in the schema, flat (see §3.4).

### 4.3 `lld.v1` names three — actually four — different shapes

| Source | What `lld.v1` denotes there |
|---|---|
| `02` §2.2 heading | *"The `ModuleBrief` schema (this becomes the `lld.v1` contract)"* → the brief |
| `03` §6 | `LldReady { schema_version, freeze, sow_seed }` → the hand-off wrapper |
| `orb/contracts/lld.v1.json:194` | `"$ref": "#/$defs/moduleBrief"` → the brief |
| `orb/.../lld-v1.ts:117` | `export type LldV1 = { brief, freeze }` → **a fourth shape, in neither spec** |

Six downstream features read this name. Left alone, each inherits a different meaning. Fix: three
files, three names (§5).

### 4.4 `content_hash` is an unlabelled 256-bit hex string in an estate that runs two hash algorithms

- `03` §4.1 prose says *blake3*; its schema says bare `^[0-9a-f]{64}$`.
- The estate's decision is **SHA-256** (FLEET-LEARNINGS U1, for real reasons: no blake3 anywhere in
  the orb, and RN/Metro cannot load `node:crypto`). **This lane carries that decision forward and does
  not re-litigate it.**
- fleet's own contracts already solved the ambiguity by prefixing:
  `receipt.v1.json` → `^blake3:[0-9a-f]{64}$`, `lane-status.v1.json` → same.
- **And the ambiguity has already produced a wrong mapping.** `03` §6's hand-off table maps
  `content_hash` → `subject.digest.blake3`. `fleet/contracts/attestation.v1.json` *requires*
  `digest.blake3` (`crew/schema.py:29`, pattern `^[0-9a-f]{64}$`). In-toto DigestSet keys **are** the
  algorithm name. Following §6 literally writes a SHA-256 digest under a key named `blake3` — a false
  statement inside a signed attestation, and one nothing would catch, because both algorithms produce
  64 hex characters.

**Fix, inside F02's scope:** `content_hash` is `^sha256:[0-9a-f]{64}$` — algorithm carried in the
string, matching fleet's existing prefix convention. This is a **named divergence from `03` §4.1's
literal pattern**, and the reason is that the literal pattern is what allowed the §6 mis-mapping. With
the prefix, an alg/digest disagreement is unrepresentable (`kills structural`) rather than merely
unlinted; the §6 mapping becomes `subject.digest[alg] = hex` and yields `digest.sha256`, correctly.

**Out of F02's scope, flagged for F07/F09:** `attestation.v1.json`'s `digest.required: ["blake3"]` will
need a `sha256` key admitted. Do **not** edit `attestation.v1.json` in this lane — it is another
contract, and contracts are human-merge. Record it in §10.1.

### 4.5 (Not a defect in F02's files, but the reason F02 exists) — live drift in fleet today

`fleet/contracts/receipt.v1.json:15` declares 7 `event` values; `fleet/crew/crew/schema.py:87-94`
mirrors 6. `lane_status` is missing. Nothing tests the binding. See §3.2. Do not fix it here — it is
`receipt.v1`'s owner's problem — but the builder should read it once, because it is what happens to
this contract in three months without §7.4.

---

## 5. The interface — exactly what ships

Three schemas in `fleet/contracts/`, mirroring fleet's existing one-file-per-contract convention,
draft 2020-12 (`fleet/contracts/*.json` all use it; the orb file's draft-07 is the outlier), `$id`
under `https://fleet.local/contracts/`, `additionalProperties:false` everywhere, `schema_version`
`const "1.0"` on every top-level object, and fleet's **AUTHORED / STAMPED** annotation discipline
(`lane-status.v1.json` is the model).

### 5.1 `fleet/contracts/module-brief.v1.json` — the proposer-authored half (`02` §2.2)

Port `orb/contracts/lld.v1.json`'s `$defs` verbatim except where listed. Root `$ref` →
`#/$defs/moduleBrief`. Every `$defs` entry is carried across unchanged:
`grain`, `registryVerdict`, `derivation`, `guarantee`, `killedAlt`, `failureStory`, `interfaceDecl`,
`dataDecl`, `acceptanceLine`, `moduleBrief`.

Changes from the orb file, and only these:

| Field | Orb today | Ships as | Why |
|---|---|---|---|
| `$schema` | draft-07 (`:2`) | `https://json-schema.org/draft/2020-12/schema` | match the other 4 fleet contracts |
| `$id` | `adhd-focus-orb.internal/...` (`:3`) | `https://fleet.local/contracts/module-brief.v1.json` | schema ownership moved (§3.1) |
| `moduleBrief.alternatives` | no `minItems` (`:150-154`) | `"minItems": 2` | §3.4, §4.2 |
| `moduleBrief.schema_version` | absent | `{"const": "1.0"}`, **required** | every fleet contract has one |

`02` §2.2 lists no `owner` and no `acceptance.artifact`; the orb file has both. **Keep both.** `03`
§4.1 requires `freeze.owner` and `accepts_when.artifact`, so the brief must be able to source them;
the orb's version already reconciled that gap between the two blueprint files. Record it in the
schema's `description` so the next reader does not "fix" it back.

**`ModuleBrief` carries zero numeric fields, and that is load-bearing** (§6.3). Every leaf is a string,
an enum, or an array/object of strings. The schema must state this as a rule, not leave it as an
accident, so a future field addition is a deliberate act:

```json
"description": "INVARIANT (hashing): no property of moduleBrief or any of its $defs may be
  type 'number' or 'integer'. content_hash is computed over canonical_json(module_brief); the
  canonicalizer REFUSES numbers (freeze.v1 §hashing) because JSON number encoding diverges across
  languages -- JSON.stringify(1.0)==='1' in JS, json.dumps(1.0)=='1.0' in Python,
  serde_json 1.0f64 -> '1.0' in Rust. Adding a numeric field here silently splits the hash three
  ways. If one is ever required, see the RFC 8785 revive trigger in the F02 lane contract §3.6."
```

### 5.2 `fleet/contracts/freeze.v1.json` — the gate-authored half (`03` §4.1)

`required`: `["schema_version","freeze_id","version","node_id","content_hash","decision","why",
"killed_alternatives","accepts_when","owner","depth_evidence","supersedes","stamped_by"]`

| Property | Type / constraint | Source & note |
|---|---|---|
| `schema_version` | `{"const":"1.0"}` | fleet convention |
| `freeze_id` | `"^fz-[0-9a-f]{16}$"` | **divergence from `03` §4.1's `id`.** `id` is unpatterned there and collides with `sow.rs`'s `id`; the orb's patterned `freeze_id` is strictly stronger. STAMPED. |
| `version` | `integer`, `minimum: 1` | `03` §4.1 verbatim. STAMPED. |
| `node_id` | `"^[a-z0-9][a-z0-9-]{2,63}$"` | must equal `module_brief.node_id`. AUTHORED (copied). |
| `content_hash` | `"^sha256:[0-9a-f]{64}$"` | **divergence, §4.4.** STAMPED. |
| `decision` | `string`, `minLength:1` | `03` §4.1. |
| `why` | `string`, `minLength:1` | `03` §4.1. |
| `killed_alternatives` | `array`, **`minItems: 2`**, items `$ref killedAlt` | `03` §4.1's `rejected`, renamed (FEATURES.md's own wording) + the missing floor restored (§4.2). |
| `accepts_when` | `object`, `additionalProperties:false`, `required:["predicate","artifact"]`; `predicate` = `{given,when,then,oracle_kind}` (`additionalProperties:false`), `artifact` = `string minLength:1` | **reverts the orb's `acceptance_test` name to `03` §4.1's.** `03` §6 maps `accepts_when.{predicate,artifact}` by name into the blind-suite seed; F07 reads it. |
| `owner` | `string`, `minLength:1` | resolves in `owners.v1.json`. |
| `depth_evidence` | `object`, `additionalProperties:false`, `required:["score","checked_check_ids"]`; `score` = the existing `depthScore` shape; `checked_check_ids` = `array<string>` | `03` §4.1's name (*"the R17–R21 block the gate validated"*). Nesting the existing `DepthScore` keeps `orb/apps/mobile/src/build/BuildEnvelope.ts:14`'s `import type { DepthScore }` compiling. F06 fills it. |
| `supersedes` | `["string","null"]`, pattern `^fz-[0-9a-f]{16}$` when string, **required** | `03` §4.1 has it optional; required-and-nullable is stronger (you cannot forget it). Named divergence, a tightening. |
| `stamped_by` | `{"const": "keel:lld-ready"}` | **§4.1 fix.** |

`depth_evidence.score` is the **only** place numbers live in this contract (`checks_total`,
`checks_passed`, `ratio`). That is why the freeze is never itself hashed — see §6.3.

### 5.3 `fleet/contracts/lld.v1.json` — the hand-off wrapper, and *only* that (`03` §6)

This name now means exactly one thing: the S1 seam message.

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://fleet.local/contracts/lld.v1.json",
  "title": "lld.v1 - the ONLY thing that crosses the orb->fleet seam (03 §6)",
  "type": "object", "additionalProperties": false,
  "required": ["schema_version","module_brief","freeze","sow_seed"],
  "properties": {
    "schema_version": {"const": "1.0"},
    "module_brief": {"$ref": "https://fleet.local/contracts/module-brief.v1.json"},
    "freeze":       {"$ref": "https://fleet.local/contracts/freeze.v1.json"},
    "sow_seed": {
      "type": "object", "additionalProperties": false,
      "required": ["restatement","blind_suite_seed","blast_radius","owner","registry_verdict"],
      "properties": {
        "restatement":      {"type": "string", "minLength": 1},
        "blind_suite_seed": {"$ref": "https://fleet.local/contracts/freeze.v1.json#/$defs/acceptsWhen"},
        "blast_radius":     {"type": "string", "minLength": 1},
        "owner":            {"type": "string", "minLength": 1},
        "registry_verdict": {"$ref": "https://fleet.local/contracts/module-brief.v1.json#/$defs/registryVerdict"}
      }
    }
  }
}
```

`03` §6's `LldReady` carries `freeze` + `sow_seed` only. **`module_brief` is added, and required.**
Reason: `freeze.content_hash` is the hash of the brief; a consumer that receives only the freeze can
never verify that hash — it has nothing to hash. A self-verifying message is worth one extra field,
and it makes the round-trip test in §7.4 possible at all. Named divergence from `03` §6.

`sow_seed`'s five fields are `03` §6's five, verbatim.

**Cross-file `$ref` note for the builder:** the TS and Rust mirrors are hand-written and do not resolve
`$ref` at runtime, so this costs nothing there. If the Python mirror ever loads these files through
`jsonschema`, it needs a local `RefResolver` store — it does not today (pydantic mirror), and F02 does
not add one.

### 5.4 Mirror placement, and why it is not what FEATURES.md says

FEATURES.md says *"TS mirror (orb) + Rust/Python mirrors (fleet)"*. **The Python mirror belongs in the
orb, not fleet.** There are exactly four runtimes that must read this contract, and mirror count is a
consequence of runtime boundaries, not a design choice:

| Runtime | Package | Has an `lld` caller today? | F02 ships a mirror? |
|---|---|---|---|
| RN / Metro (TS) | `orb/apps/mobile` | yes — `LldReadyGate.ts:17`, `BuildEnvelope.ts:14` | **yes** — adapt `lld-v1.ts` in place |
| orb relay (Py 3.12, pydantic) | `orb/backend/relay-py` | yes — `lld_decomposer.py:24` imports `ModuleBrief, validate_module_brief` (this is F03's atomizer) | **yes** — adapt `lld_schemas.py` in place |
| fleet keel (Rust) | `fleet/keel/fleet` | no — F06's gate and F07's intake will be the callers | **yes** — new `src/lld.rs` |
| fleet crew (Py 3.11, pydantic) | `fleet/crew` | **no** — `crew.sow` takes `--task <string>` today | **no — deferred to F07** |

`orb/backend/relay-py` and `fleet/crew` are separate packages with separate venvs and cannot share a
module. A fourth mirror with no caller is dead code that will drift before it is ever used (§4.5).
When F07 extends `crew.sow` to take an `lld.v1`, it adds `fleet/crew/crew/lld.py` in
`crew/schema.py`'s style **and joins the same fixture corpus and the same comparator** — that is a
hard requirement on F07, recorded in §10.1.

Three mirrors, matching FEATURES.md's acceptance line exactly. Only the repo attribution changes.

### 5.5 Files owned by this lane (no overlap with F01 or F08)

**New — `fleet/`:**
```
fleet/contracts/module-brief.v1.json
fleet/contracts/freeze.v1.json
fleet/contracts/lld.v1.json                     (replaces the meaning, not the file, of orb's)
fleet/contracts/fixtures/lld/complete_module.json      (moved from orb, adapted)
fleet/contracts/fixtures/lld/one_line_freeze.json      (moved from orb, adapted)
fleet/contracts/fixtures/lld/complete_freeze.json      (new — freeze.v1 good case)
fleet/contracts/fixtures/lld/forged_freeze.json        (new — stamped_by: "orb:lld-ready")
fleet/contracts/fixtures/lld/one_alternative.json      (new — the §3.4 regression)
fleet/contracts/fixtures/lld/numeric_trap.json         (new — the §6.3 regression)
fleet/contracts/fixtures/lld/AUTHORSHIP.md             (moved from orb, extended)
fleet/keel/fleet/src/lld.rs
fleet/keel/fleet/tests/f02_lld_crosslang.rs
fleet/tests/acceptance/lld-crosslang.sh
```
**Edited — `fleet/`:** `keel/fleet/Cargo.toml` (+`sha2 = "0.10"`), `keel/fleet/src/lib.rs`
(+`pub mod lld;`), `verify.sh` (+one `stage` line), `contracts/` README if one exists.

**Edited — `orb/`:** `apps/mobile/src/build/lld-v1.ts`, `apps/mobile/src/build/lld-v1.test.ts`
(extend only), `backend/relay-py/src/orb_relay/proxy/lld_schemas.py`,
`backend/relay-py/tests/test_lld_schema_conformance.py`.
**Deleted — `orb/`:** `contracts/lld.v1.json` (replaced by a pointer note),
`apps/mobile/src/build/fixtures/*.json` (moved to fleet; the TS test reads the fleet path).
**Moved — `orb/contracts/owners.v1.json`** → `fleet/contracts/owners.v1.json`.

**Overlap check against the live lanes.** F01 owns
`orb/apps/mobile/src/runtime/{T0FocusSession,LaunchMode}.ts`, `orb/apps/mobile/src/App.tsx`,
`orb/apps/mobile/src/build/BuildModeLaunch.test.ts` — **no intersection**. F08 owns
`fleet/keel/fleet/src/lifecycle.rs`, `src/main.rs`, `tests/f08_pr_emit.rs`, `tests/compile_fail/` —
**no intersection**, but both lanes edit `fleet/keel/fleet/Cargo.toml` and `fleet/verify.sh`. Those
two are one-line-append conflicts; see §9.

### 5.6 The Rust mirror — style is not optional

`fleet/keel/fleet/src/lld.rs` is a **hand-written `serde_json::Value` validator**, structurally
matching `validate_lane_status_projection` (`fleet/keel/fleet/src/main.rs:3526-3570`): `&Map<String,
Value>` in, `Result<(), String>` out, one early-return per rule with a human-readable reason. No
`jsonschema` crate. `pub mod lld;` goes in `src/lib.rs` (alongside `agent`/`lifecycle`/`skills`);
`main.rs` reaches it as `use fleet::lld;` — the convention `main.rs:13`/`:54` already uses.

Unlike the TS/Python mirrors, the Rust mirror returns **all** errors, not the first — the comparator
(§7.4) diffs error-path *sets* across languages, and a first-error-only validator cannot participate.
Signature: `pub fn validate_module_brief(v: &Value) -> Vec<Violation>` where
`pub struct Violation { pub path: String, pub message: String }`.

---

## 6. Canonical JSON and hashing — carried forward, plus the trap closed

### 6.1 The decision, not re-litigated

**SHA-256 over key-sorted canonical JSON, hex, prefixed `sha256:`.** Chosen by the U1 unit
(FLEET-LEARNINGS `2026-09-02-2200`) for reasons that still hold in Light: no blake3 dependency
anywhere in the orb, and `lld-v1.ts` is bundled by Metro into RN app code where `node:crypto` does
not exist. The prefix is the only change (§4.4). Rust gets `sha2` (§3.5); Python keeps
`hashlib.sha256`; TS keeps its FIPS-vector-checked implementation (`lld-v1.ts:355-409`).

### 6.2 What is hashed

**`content_hash = "sha256:" + hex(sha256(canonical_json(module_brief)))`. The brief, and nothing
else — ever.** The freeze is not hashed; the wrapper is not hashed. This is already the orb's rule
(`lld-v1.ts:325` — *"freeze_bytes = canonical_json(brief) only"*); this contract makes it a stated
invariant with a test behind it, because §6.3 depends on it.

`canonical_json`: recursive key-sort on objects, array order preserved, no whitespace, `"` escaped
per JSON. Identical in all three mirrors (`lld-v1.ts:334-344` is the reference implementation).

### 6.3 The numeric-encoding trap — closed structurally, not documented

FLEET-LEARNINGS names it: `JSON.stringify(1.0)` → `"1"` in JS, `json.dumps(1.0)` → `"1.0"` in Python.
The Rust third case: `serde_json::to_string(&1.0f64)` → `"1.0"`. **Rust agrees with Python and
disagrees with TS**, so a two-language test would have caught nothing and a three-language test
catches it 1-in-3 of the time you look.

The entry says *"`ModuleBrief` per `02` §2.2 doesn't have one today."* Verified true — every leaf of
`moduleBrief` is a string, enum, or array/object of strings. **But the trap is already armed one step
away, and the entry does not say so:**

```
orb/apps/mobile/src/build/lld-v1.ts:93-98
  export interface DepthScore {
    readonly checks_total: number;
    readonly checks_passed: number;
    readonly ratio: number;          // checks_passed / checks_total, 3dp   ← a float
    ...
  }
```

`FreezeRecord.version` is an integer and `depth_evidence.score.ratio` is a float. The instant anyone
hashes a `FreezeRecord` — which is a natural thing to want, and `lld-v1.ts:117`'s stray
`LldV1 = {brief, freeze}` type suggests someone was already thinking about it — a `ratio` of exactly
`1.0` serializes as `1` in TS and `1.0` in Python and Rust, and the hashes silently diverge. The trap
is not hypothetical-and-distant; it is one refactor away, in a field that already exists.

**Fix — make the encoding unrepresentable rather than correct.** `canonical_json` in all three mirrors
**refuses** a JSON number:

- TS: `canonicalJson` throws `TypeError('canonicalJson: numbers are not canonicalizable (F02 §6.3)')`
  before reaching the `JSON.stringify(value)` fall-through at `lld-v1.ts:343`.
- Python: `canonical_json` raises `TypeError` with the same message.
- Rust: `canonical_json` returns `Err(CanonicalError::NumericLeaf { path })`.

Ordinary serialization is untouched — `depth_evidence.score.ratio` still stores and transmits as a
number through normal `JSON.stringify` / `json.dumps` / `serde_json`. Only the *hashing* serializer
refuses. This is `kills (structural)`: with no number encodable, no encoding divergence exists.

`numeric_trap.json` (§7.4) is a `module_brief` with `depth_evidence` grafted on, containing
`"ratio": 1.0`. All three mirrors must refuse to hash it, identically. The fixture is the regression
test that stops a future field addition from silently reopening this.

---

## 7. Acceptance suite — write these first, they are the spec

**T1/T2.** Every test below is written *before* the implementation and is expected to be **red** until
the builder makes it green. **The builder may not edit any file in this section.** Any diff touching
`orb/apps/mobile/src/build/lld-v1.test.ts`,
`orb/backend/relay-py/tests/test_lld_schema_conformance.py`,
`fleet/keel/fleet/tests/f02_lld_crosslang.rs`, or `fleet/tests/acceptance/lld-crosslang.sh` beyond
*adding* cases is a contract breach — flag it, do not merge it.

The five existing TS cases (`lld-v1.test.ts` U1-T1..U1-T5) are **carried forward unchanged** except
for the fixture path. They already encode good properties (key-order invariance, 100-shuffle
stability, forbidden-field rejection); weakening them to make a new schema pass is a breach.

### 7.1 The fixture corpus — `fleet/contracts/fixtures/lld/`

One corpus, read by all three mirrors by relative path. No mirror carries its own copy.

| Fixture | Shape | Must |
|---|---|---|
| `complete_module.json` | `module_brief` | validate OK in all 3 |
| `one_line_freeze.json` | `module_brief` | fail in all 3, naming ≥3 distinct paths, **the same set in all 3** |
| `one_alternative.json` | `module_brief`, `grain:"leaf"`, one `alternatives` entry | fail in all 3 on path `alternatives` (§3.4 regression) |
| `complete_freeze.json` | `lld.v1` wrapper | validate OK in all 3; `freeze.content_hash` must equal the recomputed hash of its own `module_brief` |
| `forged_freeze.json` | `lld.v1` wrapper, `stamped_by: "orb:lld-ready"` | fail in all 3 on path `freeze.stamped_by` (§4.1 regression) |
| `numeric_trap.json` | `module_brief` + a grafted `depth_evidence.score.ratio: 1.0` | **hashing must refuse** in all 3, identically (§6.3 regression) |

`AUTHORSHIP.md` carries forward from `orb/apps/mobile/src/build/fixtures/AUTHORSHIP.md` and gains a row
per new fixture. The existing rule stands: the bad fixtures must not be authored by the same unit that
writes the gate (F06). These four new ones are **authored by the lead in this contract**, i.e. by
neither the F02 builder nor F06's — record that; it is a stronger position than U1 had.

### 7.2 TS — `orb/apps/mobile/src/build/lld-v1.test.ts`

Keep U1-T1..U1-T5 verbatim, repoint `load()` at the fleet corpus, then add:

```ts
const FIX = `${__dirname}/../../../../../fleet/contracts/fixtures/lld`;
const load = (n: string) => JSON.parse(readFileSync(`${FIX}/${n}.json`, 'utf8'));

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
```

`crossLangReport(dir)` is the one new exported function the TS mirror must provide — see §7.4 for its
exact required output.

### 7.3 Python — `orb/backend/relay-py/tests/test_lld_schema_conformance.py`

Same six new cases, same fixture corpus, `pytest`. Plus one Python-only case that the TS side cannot
express:

```python
def test_f02_t13_pydantic_forbids_extra_on_every_nested_model():
    """extra='forbid' on the outer model does not imply it on nested ones.
    additionalProperties:false is on EVERY $defs entry in the schema; assert the mirror matches."""
    for model in (ModuleBrief, Freeze, LldV1, Guarantee, KilledAlt, FailureStory,
                  InterfaceDecl, DataDecl, AcceptanceLine, DepthScore):
        assert model.model_config.get("extra") == "forbid", model.__name__
```

and the report emitter, mirroring F02-T12.

### 7.4 The cross-language comparator — the test the acceptance line is actually asking for

FEATURES.md's *"round-trips through all 3 language mirrors byte-identical"* becomes three testable
claims. **Critically: no language asserts against a hardcoded constant.** Each mirror emits a report;
one comparator diffs the three files. A hardcoded expected hash in each language would let all three
be wrong together and still pass — the estate's own "gate passed on the defect's own data" failure.

Each mirror exposes a report emitter producing **exactly** this shape, with object keys sorted and a
trailing newline:

```json
{
  "mirror": "ts" | "py" | "rs",
  "contract": "lld.v1",
  "fixtures": {
    "<fixture-name>": {
      "valid": true|false,
      "error_paths": ["sorted","unique","list"],
      "canonical_json": "<the exact canonicalizer output, or null if refused>",
      "content_hash":   "sha256:<64hex>, or null if refused",
      "hash_refusal":   "<error message, or null>"
    }
  }
}
```

`fleet/tests/acceptance/lld-crosslang.sh` — a `stage` in `fleet/verify.sh`, matching the existing
`tests/acceptance/*.sh` pattern:

```bash
#!/usr/bin/env bash
# F02: three mirrors, one corpus, one comparison. Exit 6 on any divergence (fleet's invariant code).
set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"          # -> fleet/
LIGHT="$(cd "$ROOT/.." && pwd)"                       # -> Light/
OUT="$(mktemp -d)"; trap 'rm -rf "$OUT"' EXIT

# 1. Each mirror emits its own report. Statuses captured directly -- never $? after a pipe (E1/S11).
( cd "$LIGHT/orb" && F02_REPORT_OUT="$OUT/ts.json" npx vitest run apps/mobile/src/build/lld-v1.test.ts ) \
  >"$OUT/ts.log" 2>&1; ts_rc=$?
( cd "$LIGHT/orb/backend/relay-py" && F02_REPORT_OUT="$OUT/py.json" .venv/bin/pytest -q tests/test_lld_schema_conformance.py ) \
  >"$OUT/py.log" 2>&1; py_rc=$?
( cd "$ROOT/keel" && F02_REPORT_OUT="$OUT/rs.json" cargo test --quiet --test f02_lld_crosslang ) \
  >"$OUT/rs.log" 2>&1; rs_rc=$?

for pair in "ts:$ts_rc" "py:$py_rc" "rs:$rs_rc"; do
  case "$pair" in *:0) ;; *) echo "MIRROR FAILED: ${pair%%:*} (see $OUT/${pair%%:*}.log)"; exit 6 ;; esac
done

# 2. MEASURING NOTHING IS A FAILURE. A missing/empty report must not read as agreement.
for m in ts py rs; do
  [ -s "$OUT/$m.json" ] || { echo "NO REPORT from $m -- three mirrors must all report"; exit 6; }
done
n=$(python3 -c 'import json,sys; print(len(json.load(open(sys.argv[1]))["fixtures"]))' "$OUT/ts.json")
[ "$n" -ge 6 ] || { echo "corpus shrank to $n fixtures (expected >= 6)"; exit 6; }

# 3. The comparison. Reports are byte-identical except the "mirror" key.
for m in py rs; do
  if ! diff -u <(sed 's/"mirror": "ts"/"mirror": "X"/' "$OUT/ts.json") \
               <(sed "s/\"mirror\": \"$m\"/\"mirror\": \"X\"/" "$OUT/$m.json"); then
    echo "MIRROR DIVERGENCE: ts vs $m -- $n fixtures compared"; exit 6
  fi
done
echo "ok  3 mirrors agree on $n fixtures"
```

Three properties this asserts that no per-language test can:

1. **Same verdict** — `valid` matches for all 6 fixtures across 3 mirrors.
2. **Same reasons** — `error_paths` sets match. A mirror that rejects the bad fixture for a
   *different reason* is a drift the acceptance line as written would have missed.
3. **Same bytes, then the same hash** — `canonical_json` is compared *before* `content_hash`, so a
   divergence is localized to the canonicalizer rather than surfacing as two opaque hex strings. And
   `hash_refusal` must match, which is what makes the §6.3 numeric guard cross-language-proven rather
   than three independent claims.

The count of compared fixtures is printed on success. A comparator that silently compares zero
fixtures is the failure this contract is most concerned about.

### 7.5 Rust — `fleet/keel/fleet/tests/f02_lld_crosslang.rs`

`use fleet::lld;`. Cases F02-T6..T11 as above plus the report emitter, and one Rust-only case:

```rust
#[test]
fn f02_t14_no_source_path_constructs_the_stamper_literal() {
    // The const exists so a proposer cannot forge it. Assert the retirement, not just the
    // replacement: nothing in the crate may WRITE the string; only compare against it.
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lld.rs")).unwrap();
    let writes = src.matches("\"keel:lld-ready\"").count();
    let compares = src.matches("== \"keel:lld-ready\"").count()
                 + src.matches("STAMPED_BY").count();
    assert!(writes > 0, "the const must exist");
    assert_eq!(writes, compares, "stamper literal is constructed, not only compared");
}
```

### 7.6 Prove the suite red before trusting it

Before the builder starts, and recorded in the evidence bundle: run §7.4 against the tree **as it
stands**. It must fail — `fleet/contracts/module-brief.v1.json` does not exist, `src/lld.rs` does not
exist, and there is no report emitter anywhere. Capture that output. A suite never observed red is a
suite that might be asserting nothing (see F02's own §4.5 for what an unasserted binding looks like
after three months).

Then, after green, mutation-check the two that matter most, one at a time, reverting each:
- flip `stamped_by`'s const back to `orb:lld-ready` → F02-T7 and the comparator must both go red;
- delete the numeric refusal from the **TS mirror only** → F02-T9 goes red *and* the comparator
  reports a `ts` vs `py`/`rs` divergence on `numeric_trap`. If only one of those two fires, the
  comparator is not comparing what it claims to.

---

## 8. Done-definition

A checkbox is not done until its evidence line is in the bundle.

1. `fleet/contracts/{module-brief,freeze,lld}.v1.json` exist, draft 2020-12, `additionalProperties:false`
   on every object, `schema_version` const on every top-level, AUTHORED/STAMPED annotated.
2. All four §4 defects fixed: `stamped_by: "keel:lld-ready"`; `minItems: 2` on both alternatives
   arrays; three names, three files; `content_hash` prefixed `sha256:`.
3. Three mirrors exist and are adapted **in place** where they already existed (`lld-v1.ts`,
   `lld_schemas.py`) — a new parallel file next to an old one is a fail.
4. `orb/contracts/lld.v1.json` is deleted and replaced by a one-line pointer note; `grep -rn
   "adhd-focus-orb.internal"` returns 0 hits in source.
5. `bash fleet/tests/acceptance/lld-crosslang.sh` exits 0 and prints `3 mirrors agree on >= 6 fixtures`.
6. The §7.6 red-first output and both mutation results are in the evidence bundle. Not "tests pass" —
   the actual captured output, including the red one.
7. `bash fleet/verify.sh` — full result pasted, red included. Any pre-existing failure isolated by a
   **disposable detached worktree**, never `git stash` (§11).
8. `cd orb && npm run verify` — same rule. The orb's typecheck is expected to be red on pre-existing
   `@pe/*` module-resolution gaps; isolate and diff the failure *set*, not the exit code.
9. `cd fleet/crew && uv run pytest -q` green (F02 does not touch it, but `verify.sh:98` runs it).
10. `FEATURES.md` F02 row updated: registry column `build-new` → `extract (C2) + build-new`, with the
    §2 reason in one line.
11. A dated `FLEET-LEARNINGS.md` entry appended (absolute path — never a relative guess from inside a
    worktree; that mistake has been made twice, see the U1 entry).
12. **PR opened, not merged.** Contract + it changes what CI accepts → A15/D4 human-merge. No
    self-approve.

---

## 9. Handoff — sequencing, and which repo to start in

### 9.1 The build must not start yet

F01 (`orb`, `STATUS=building` since 01:35) and F08 (`fleet`, `STATUS=building` since 01:34) are both
live. This build's rule is **one feature building at a time per repo**, and F02 needs *both*. Contract
authoring touches no production code, so producing this document now is safe; the build waits.

### 9.2 Start in **fleet** — and the reason is not lane availability

Even if the orb lane frees first, do not start the orb half.

1. **The dependency runs one way.** `fleet/contracts/*.v1.json` and the fixture corpus are what the
   orb's TS and Python mirrors are adapted *to*. Starting in the orb means editing two mirrors against
   a schema that does not exist yet, then editing them again. Sequencing follows the dependency graph,
   not the lane calendar.
2. **The fleet half is almost entirely new files** — 12 new, 3 one-line edits. New files cannot
   conflict with F08. The orb half is *all* edits to files that already exist.
3. **The comparator lives in fleet** and is the only artifact that can prove the orb half correct. It
   has to exist before the orb half can be verified.

**Two-phase brief, sized at roughly one focused session each:**

| Phase | Repo | Scope | Gate to advance |
|---|---|---|---|
| **F02a** | `fleet` | 3 schemas, 6 fixtures, `src/lld.rs`, `Cargo.toml` +`sha2`, `lib.rs` +`pub mod lld`, `tests/f02_lld_crosslang.rs`, `tests/acceptance/lld-crosslang.sh`, `verify.sh` stage | `cargo test --test f02_lld_crosslang` green; comparator exits 6 with `MIRROR FAILED: ts` (correct — the orb half is not built) |
| **F02b** | `orb` | adapt `lld-v1.ts` + `lld_schemas.py` in place, repoint fixtures, extend both test files, delete `orb/contracts/lld.v1.json` | comparator exits 0, `3 mirrors agree on 6 fixtures` |

F02a can start the moment F08's lane frees. F02b needs F01's lane *and* F02a merged to the F02 branch.

### 9.3 Routing (A17)

| Phase | Model | Why |
|---|---|---|
| F02a | **mid-engineer (Sonnet)** | new Rust module + a shell comparator + a schema split with four judgment calls behind it. Not mechanical. |
| F02b | **mid-engineer (Sonnet)** | in-place edits to two working validators, both with cross-field predicates, in a repo whose test env has four documented traps (§11). |
| Verify | **verifier (Sonnet, never Haiku)** | must independently re-derive §7.6 red-first + both mutations in a clean worktree. |

Do not route either phase to junior-engineer. The schema is copy-shaped and looks mechanical; the four
§4 fixes are not, and the numeric refusal (§6.3) is the kind of thing that gets "simplified" back out
by someone who does not know why it is there.

### 9.4 Two files both lanes append to

`fleet/keel/fleet/Cargo.toml` and `fleet/verify.sh` are appended to by both F02a and F08. Each is a
single added line at the end of an existing list. Rebase F02a onto F08 rather than the reverse (F08
started first and is further along), and re-run `fleet/verify.sh` after — do not assume an append
conflict resolves cleanly just because the merge did.

---

## 10. Assumptions, and what this lane deliberately does not do

### 10.1 Flagged, not fixed — hand these forward

| # | Finding | Owner |
|---|---|---|
| 1 | `fleet/contracts/attestation.v1.json`'s `subject.digest.required: ["blake3"]` cannot accept a SHA-256 content hash. `03` §6's mapping `content_hash → subject.digest.blake3` is wrong once the estate chose SHA-256 (§4.4). | **F07 / F09.** Editing another contract is human-merge and out of this lane. |
| 2 | `fleet/contracts/receipt.v1.json:15` (7 events) vs `fleet/crew/crew/schema.py:87-94` (6 — no `lane_status`). Live drift, no binding test (§4.5). | `receipt.v1`'s owner. Worth a one-line spawn. |
| 3 | `fleet/crew/crew/lld.py` — the 4th mirror. Must join this corpus and this comparator when F07 needs it (§5.4). | **F07, hard requirement.** |
| 4 | `orb/apps/mobile/src/build/LldReadyGate.ts` implements 14 checks against the *old* shape and will not compile after F02b. **And it carries its own second copy of the §3.4 loophole** — `LldReadyGate.ts:134-135`: `const floor = b.grain === 'leaf' ? 1 : 2; if (b.alternatives.length < floor) return false;`. Closing it in the schema and the three mirrors leaves it open in the gate. | **F06** re-homes the gate to fleet eventually, but F02b must (a) leave it compiling — adapt the import and the renamed fields — and (b) delete the `grain` floor there too, or the R21 fix is cosmetic. **Both explicitly in F02b's scope.** |
| 5 | The status filename in the F02 brief (`F02-lld.v1-contract-schema+TS+Rust+Python-mir.status`) does not exist on disk. The real file is `F02-lld-v1-contract--schema-TS-Rust-Python-m.status`. | Noted; the real file is the one written. |

### 10.2 Assumptions

- FEATURES.md's `build-new` and its `Python mirror (fleet)` attribution are both superseded by §2 and
  §5.4. If the orchestrator disagrees, that is a lead-level call to reverse before F02a starts, not
  something the builder should resolve.
- `03` §4.1 is treated as authoritative on **names** (`accepts_when`, `depth_evidence`) and the orb's
  copy as authoritative on **constraints** (patterns, required-nullable). Each divergence in §5.2 is
  individually named and justified; none is silent.
- The `sha256:` prefix (§4.4) is the one change to a blueprint-literal schema in this contract. It is
  called out for the human reviewer specifically.

### 10.3 Explicitly out of scope

- The `lld-ready` gate itself (F06). This lane ships the *shapes*; nothing here decides whether a
  brief is ready.
- Any codegen (F35).
- The design-graph node / `design-graph.v1` (`03` §3) and `lane-status.v1` push (F21).
- Wiring the seam (F09). F02 defines the message; nobody sends one yet.
- `freeze.v1`'s append-only ledger and supersede mechanics (`03` §4.2/§4.3). The *field* `supersedes`
  ships; the ledger that enforces it does not.

---

## 11. Landmines already paid for — do not rediscover

From FLEET-LEARNINGS, all confirmed applicable to this tree:

1. **Never `git stash` in any worktree here.** `refs/stash` is one shared stack across every worktree
   of a repo; a sibling lane's `pop` will take yours. This has already cost two sessions
   (`sot-lld-ready-gate`, `listening-state-delivery-receipt`). Use
   `git worktree add --detach /tmp/f02-baseline <ref>` for a baseline, or `git diff > x.patch`.
2. **Never background a slow command and wait.** A subagent gets no notification for its own
   background children and will sit forever. Foreground, or `timeout N <cmd>`.
3. **`.gitignore`'s bare `build/` swallowing `orb/apps/mobile/src/build/` — already fixed, verified,
   do not re-fix.** The negation came across in the D15 copy in both places: `Light/.gitignore:18-20`
   (`build/`, `!orb/apps/mobile/src/build/`, `!orb/apps/mobile/src/build/**`) and
   `orb/.gitignore:3-8`. Listed here only because the trap cost a prior session and the reflex on
   seeing a missing file is to reach for `git add -f`. Your files under that path *do* track.
4. **The orb's `node_modules/@pe/*` symlinks are baked at a fixed relative depth** that only resolves
   from the original checkout path. In a worktree they dangle, and `tsc`/`vitest` report
   `Cannot find module 'react'` instead of the real errors. Structural, pre-existing, not yours.
   Isolate before attributing any red to your diff.
5. **`orb`'s `npm run verify` is red at baseline** on `@pe/realtime-voice` / `@pe/llm-gateway`
   resolution. Diff the failure *set* against a disposable baseline worktree; do not read the exit
   code as a verdict on your change.
6. **A fresh worktree has no `node_modules` and no Python `.venv`.** Both are gitignored. Establish
   S0 in the worktree before editing, and record what you had to do.
7. **`fleet/verify.sh` never uses `$?` after a pipe** (its own comment, `verify.sh:20`, D22/D23).
   The comparator in §7.4 follows the same rule — captured statuses, no pipes into the check.
8. **Write `FLEET-LEARNINGS.md` to its absolute path.** Two prior units created a stray in-repo copy
   instead, and one of those was then deleted with a false "already folded in" claim. The content was
   lost until a verifier caught it.
