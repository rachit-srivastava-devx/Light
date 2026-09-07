# F07 — Fleet SOW-intake extension: `lld.v1` → a real SOW

**Lane:** `lane/F07-sow-intake` · **Repo:** `fleet` · **Depends on:** F02 (`lld.v1`), F06 (`lld-ready` gate)
**Role:** LEAD (contract only, no implementation) · **Written:** 2026-09-07
**Primary spec:** `blueprints/Speed-of-Thought-L8-Deep-Dive/03-SPEC-COMPLETE-GATE-AND-DESIGN-GRAPH.md` §6
**Secondary:** `04-DELIVERY-LANE-SDLC.md` §1–2 (S0/S1) · **Prior lanes:** `docs/lane-contracts/F02-lld-v1-contract.md`, `F06-lld-ready-gate.md`

---

## 0. BLOCKER — this worktree is not what the dispatch said it was

The dispatch stated the lane sits "on master with F01–F04, F06, F08 merged." **F06 is not in this
worktree.** Measured, not assumed:

```
lane/F07-sow-intake  = 20c857e  (Merge F03)
master               = ce6e2e3
git rev-list --count lane/F07-sow-intake..master  ->  4
```

`git diff lane/F07-sow-intake master -- fleet/` — what this lane is missing:

| file | delta |
|---|---|
| `fleet/keel/fleet/src/lld_ready.rs` | **+557 (absent entirely)** |
| `fleet/keel/fleet/tests/f06_lld_ready.rs` | +541 (absent) |
| `fleet/contracts/gate-refs.v1.json` | +6 (absent) |
| `fleet/keel/fleet/src/main.rs` | +175 (the `fleet gate lld-ready` dispatch) |
| `fleet/keel/fleet/src/lib.rs`, `lld.rs`, `verify.sh`, fixtures `AUTHORSHIP.md` | small |

F08 *is* present (`git merge-base --is-ancestor 9358ea0 master` → yes; the merge is reachable from
both `master` and this lane — it simply does not appear in `git log --oneline -12`'s topological
window, which is what made it look absent at first glance). F06 is genuinely missing.

**F07 cannot be built here as-is** — §5's pipeline calls `lld_ready::check_and_evaluate`, which does
not exist on this branch. **First builder action, before any edit:**

```
git -C <worktree> merge --ff-only master     # or: git rebase master
```

and re-confirm `fleet/keel/fleet/src/lld_ready.rs` exists. Contract-level facts below were read from
the **master** checkout (`/Users/rachitsrivastava/youtube/Principal Engineering/Light/`), not from
this stale worktree.

---

## 1. Restatement

`fleet sow` today has exactly one intake: a CLI string. `create_sow(task: &str)` shells out to
`python -m crew.sow --task <text>`, keys the result by `blake3(task)`, writes
`state/sows/<id>.json`, and exits **9** `SOW_READY_AWAITING_REVIEW` until a human runs
`fleet sow accept --id <id>`.

F07 adds a **second intake for the same machinery**: a gate-passed `lld.v1` node on disk. The node
is validated, its freeze is checked against its own brief, the readiness gate is re-run, the
registry verdict is honoured, and the result is **compiled into the exact task text the existing
`crew.sow` pipeline already consumes** — after which not one line of the exit-9 / human-accept path
changes.

**The one-sentence acceptance (FEATURES.md F07):** feeding a valid `lld.v1` fixture through the
extended intake produces the same `SOW_READY_AWAITING_REVIEW` exit fleet's CLI path already
produces. This contract makes "the same" mechanically checkable: **the same `id`.**

---

## 2. What I measured in the tree before designing (the reason this lane matters)

### 2.1 `content_hash` is pattern-checked and never recomputed. Anywhere.

`freeze.v1.json`'s own description claims the hash is *"cross-field, checked by the `lld.v1` wrapper
validator"*. **It is not.** `lld::validate_lld_v1` (`fleet/keel/fleet/src/lld.rs:705`) checks shape,
`additionalProperties`, and exactly one cross-field rule — `freeze.node_id == module_brief.node_id`.
The only `content_hash` logic in `check_freeze` is a regex:

```rust
// fleet/keel/fleet/src/lld.rs:554
if !as_str(f.get("content_hash")).is_some_and(valid_content_hash) {
    push(v("freeze.content_hash", "content_hash must match ^sha256:[0-9a-f]{64}$"));
}
```

`lld::content_hash()` exists at `lld.rs:825` and **no consumer calls it to verify a freeze.**

Consequence, today: a hand-written JSON file with `stamped_by: "keel:lld-ready"` and *any* 64-hex
string passes both `fleet contract lld validate` and `fleet gate lld-ready`. The `forged_freeze.json`
fixture only catches a forger who writes the **wrong stamp string**; a forger who copies the right one
is not caught by anything in this tree.

F07 is **the first consumer that converts an `lld.v1` into authority to spend build budget.** Closing
this is not a nice-to-have bolted onto the lane — it is the lane's load-bearing half. §7's `F07-T4`
proves the hole is real by asserting the *existing* commands still pass the forgery.

### 2.2 Blueprint 03 §6's mapping table cites two paths that do not exist in the shipped schema

| 03 §6 says | shipped reality | F07's resolution |
|---|---|---|
| `depth_evidence.blast_radius` | `freeze.depth_evidence` = `{score, checked_check_ids}` **only**. `blast_radius` lives at `sow_seed.blast_radius` and `module_brief.failure_story.blast_radius` | read `sow_seed.blast_radius` (the seed is the hand-off surface; the brief's copy is prose about a failure mode, not the lane's radius) |
| `content_hash` → `subject.digest.**blake3**` | `content_hash` is `sha256:…` — F02 §4.4 added the algorithm prefix *specifically because* 03 §6 had already produced one wrong alg/digest mapping | store the prefixed string **verbatim**; never write it into a field named `blake3`. F07 does not build the attestation (out of scope, §9) — it only carries the value forward correctly |
| `purpose` (as a freeze field) | `purpose` is on `module_brief`, not `freeze` | read `module_brief.purpose` |

### 2.3 03 §6 and 04 §1 contradict each other on the human edge

- **03 §6:** *"`lld.v1` lands the SOW at `Specified`, which exits 9 (`SOW_READY_AWAITING_REVIEW`) until
  a human accepts… The gate makes a module buildable; it never makes it accepted."*
- **04 §1:** *"The operator's spoken ❄ FREEZE confirmation **is** the `Reviewed` edge, carried into the
  lane as a field on `lld.v1`: `reviewed_by`, `reviewed_at`, `freeze_receipt_hash`."*

**None of those three fields exists** in `lld.v1`, `freeze.v1`, or `module-brief.v1`, and all three
schemas are `additionalProperties: false`, so they cannot be smuggled in. 04 §1's S0 precondition
("refuse a node with no `freeze_receipt_hash`") is **not expressible against the schema that shipped.**

**Resolution (locked): 03 §6 wins for F07.** Exit 9 and the human accept stay exactly as they are.
See killed alternative K3.

---

## 3. C1 registry verdict (C1/L2 — checked, not asserted)

**Searched:**

| where | result |
|---|---|
| `Light/registry/{features,services,modules}/` | `.gitkeep` only — three empty directories, zero installable capabilities (matches `gate-refs.v1.json`'s own honest limit note) |
| `grep -rln "sow_seed" fleet/ orb/` | `lld.rs`, `lld.v1.json`, 2 fixtures, `lld_schemas.py`, `lld-v1.ts` — all **shape validators**, no consumer |
| `grep -rln "Edge cases:\|Alternatives:" fleet/` | `crew/crew/sow.py` + its tests, and `sow.rs` (in a test string) — the **parser** exists; no producer |

**Verdict: `install` (reuse) for every mechanism; `build-new` for exactly one pure function.**

| capability | branch | source |
|---|---|---|
| SOW mechanical validation | **install** | `crew/crew/sow.py`, invoked unchanged via `sow::invoke_crew` |
| SOW identity, record write, lock, exit 9, accept | **install** | `sow.rs` + `create_sow`/`accept_sow`, unchanged |
| `lld.v1` shape validation | **install** | `lld::validate_lld_v1` (F02) |
| canonical JSON + hashing | **install** | `lld::canonical_json`, `lld::content_hash` (F02) |
| readiness gate | **install** | `lld_ready::check_and_evaluate`, `GateRefs`, `load_gate_refs` (F06) |
| refusal receipts | **install** | `refusal_with_receipt`, `append_receipt` |
| **`lld.v1` → crew.sow task text** | **build-new** | nothing renders this text anywhere; ~1 pure function |

Net new logic: one deterministic string compiler plus five guard checks. Everything else is a call.

---

## 4. Killed alternatives

**K1 — Build the SOW JSON directly in Rust from `lld.v1`; skip `crew.sow`.**
Killed: `sow.rs`'s own header states *"Mechanical SOW validation deliberately remains in `crew.sow`.
This module only invokes that source of truth."* A second, Rust-side SOW constructor forks that
validation and the two drift — F02 §4.3 documents a **live** drift between `receipt.v1.json` and
`crew/schema.py` produced by exactly this pattern. Independently: `id_for_task` is `blake3` of a task
**string**, so a task string must exist regardless; skipping crew.sow saves nothing and costs the
single source of truth.
*Revive trigger:* `crew.sow` is retired or ported to Rust as the one implementation.

**K2 — Add a Python `crew.sow --lld <path>` entry point using `build_sow()`'s structured kwargs.**
Genuinely attractive: `build_sow(leaves=…, challenges=…, alternatives=…, estimate=…, edge_cases=…)`
bypasses every text-parsing trap in §6.4. Killed on two counts: (a) it moves the `lld.v1` mapping into
Python, where `lld_schemas.py` is a *mirror* — F02 §3.1 moved schema ownership fleet-ward precisely so
the authority does not live with the mirror; (b) a task string is still required for `id_for_task`, so
the text compiler would have to exist anyway — this option **adds** a second mapping rather than
removing one.
*Revive trigger:* `SOW_TEXT_*_COLLISION` refusals (§6.5) exceed 1 in 20 real freezes — **measured over
a real freeze corpus, not estimated** — i.e. the text channel is demonstrably too lossy.

**K3 — Treat the freeze as the `Reviewed` edge and skip the human accept (04 §1).**
Killed: 03 §6 — this lane's primary spec — says the opposite, and the fields 04 §1 depends on
(`freeze_receipt_hash`, `reviewed_by`, `reviewed_at`) **do not exist** (§2.3). Auto-accepting on the
strength of an absent field is accepting on nothing. This is the human-edge-as-policy-not-type failure
that `06`'s typestate exists to prevent, and D4/A15 puts contracts and money on human-merge always.
*Revive trigger:* F02's schema gains the three Reviewed-edge fields through its own lane and a
human-merged contract change. Then **F09/F10** may collapse the edge — not F07.

**K4 — Trust `stamped_by`; don't re-run the gate or recompute the hash.**
Killed by measurement, not principle: §2.1: nothing in this tree recomputes `content_hash`, and
`stamped_by` is a plain string in a plain file. The const stops a careless forger and no one else.
*Revive trigger:* freezes arrive over an authenticated transport carrying a signature (F09/F21) — at
which point the transport, not the payload, carries authority, and F07 verifies the transport instead.

**K5 — A new top-level subcommand, `fleet lld-sow <path>`.**
Killed: it produces the same artifact in the same `state/sows/` namespace under the same `id_for_task`.
A separate command invites a separate identity space, and the first bug is two SOWs for one module.
The flag keeps one command, one namespace, one accept path.

---

## 5. Interface

### 5.1 CLI

```
fleet sow --task <T>        # unchanged
fleet sow --lld  <PATH>     # NEW — PATH is a file containing an lld.v1 wrapper
fleet sow accept --id <ID>  # unchanged; accepts either origin identically
```

`sow_command` currently matches `[flag, task]` and `[command, flag, id]`. Add one arm:

```rust
if let [flag, path] = args {
    if flag == "--lld" { return create_sow_from_lld(path); }
}
```

Help text (`main.rs` usage block) gains one line:

```
sow --lld <PATH>      Compile a gate-passed lld.v1 into a SOW; same exit 9 as --task.
```

### 5.2 Pipeline — nine steps, one of them new

| # | step | call | reuse |
|---|---|---|---|
| 1 | read + parse | `fs::read_to_string` → `serde_json::from_str` | mirrors `contract_lld_validate` |
| 2 | shape | `lld::validate_lld_v1(&value)` | F02 |
| 3 | **freeze binds to brief** | `lld::content_hash(&value["module_brief"])` **==** `value["freeze"]["content_hash"]` | **NEW guard, existing fn** |
| 4 | provenance agrees | `module_brief.owner == freeze.owner == sow_seed.owner`; `module_brief.registry` deep-eq `sow_seed.registry_verdict` | **NEW guard** |
| 5 | readiness | `lld_ready::check_and_evaluate(&value["module_brief"], &refs)` must be `Gate(Verdict{outcome: Ready})` | F06 (note: takes the **bare brief**, not the wrapper) |
| 6 | `GateRefs` | `load_gate_refs()` | F06 |
| 7 | **registry branch** | `sow_seed.registry_verdict.kind`: `install`/`extract` → short-circuit refusal; `build_new` → proceed | **NEW guard** (03 §6) |
| 8 | **compile** | `sow::compile_task_text(&value) -> Result<String, CompileError>` | **NEW — the only new logic** |
| 9 | hand off | `create_sow(&task_text)` | **unchanged, verbatim** |

Then print the compiled text to **stderr** so the operator can drive `fleet run --task "<text>"`:

```
SOW_LLD_TASK_BEGIN
<compiled text>
SOW_LLD_TASK_END
```

Delimiters are required — they are what makes `F07-T2`'s round-trip mechanically extractable.

### 5.3 Provenance carried into the record

`create_sow` writes `sow::ready_record(id, task, sow, created_at)`. Add a **sibling** constructor —
do not change `ready_record`, and do not touch `accepted_for_exact_task`:

```rust
pub fn ready_record_with_provenance(
    id: &str, task: &str, sow: Value, created_at: &str, provenance: Value,
) -> Value    // = ready_record(..) + { "lld_provenance": provenance }
```

```json
"lld_provenance": {
  "node_id": "...", "freeze_id": "fz-...", "freeze_version": 1,
  "content_hash": "sha256:...",          // verbatim, prefix intact (§2.2)
  "blast_radius": "...",                 // from sow_seed.blast_radius
  "owner": "...",
  "registry_verdict": { "kind": "build_new", "searched": [...] },
  "gate": { "checks_passed": 14, "checks_total": 14, "checked_check_ids": [...] }
}
```

`accepted_for_exact_task` reads only `id`/`task`/`accepted`, so the accept path is untouched by
construction — asserted by `F07-T13`.

---

## 6. The mapping — `lld.v1` → `crew.sow` task text

`crew.sow` is line-oriented and section-delimited (`crew/crew/sow.py:_split_sections`). The compiler
emits **exactly** this grammar, in this order, with `\n` separators and no trailing newline:

```
Task: {RESTATEMENT}
Leaves:
- {LEAF} | acceptance: {PREDICATE}
Challenges:
- {RISK} [{CITATION}]
Alternatives:
- Alt1: {OPT_1} — {WHY_KILLED_1} | tradeoff: {REVIVE_1}
- Alt2: {OPT_2} — {WHY_KILLED_2} | tradeoff: {REVIVE_2}
Estimate:
- not estimated at freeze time (lld.v1 carries no estimate field; F07 contract §6.4)
Edge cases:
- {FAIL_SAFE}
- {NON_GOAL_1}
```

### 6.1 Field sources (03 §6's table, resolved against the shipped schema)

| slot | source | 03 §6 row |
|---|---|---|
| `RESTATEMENT` | `sow_seed.restatement` + `" Decision: "` + `freeze.decision` + `" Why: "` + `freeze.why` + `" Purpose: "` + `module_brief.purpose` | decision + why + purpose → SOW restatement |
| `LEAF` | `"Given {p.given}, when {p.when}, then {p.then}"` from `sow_seed.blind_suite_seed.predicate` | accepts_when → blind-suite seed |
| `PREDICATE` | `"{p.oracle_kind}:{blind_suite_seed.artifact}"` | same row |
| `RISK` | `module_brief.failure_story.trigger` | — (the only risk statement in the node) |
| `CITATION` | first `freeze.depth_evidence.checked_check_ids[i]` whose leading token matches `[A-Z]{1,3}\d+`, emitted **verbatim** (e.g. `C1-OPEN`) | — (see §6.3) |
| `OPT_n` / `WHY_KILLED_n` / `REVIVE_n` | `freeze.killed_alternatives[n].{option, why_killed, revive_trigger}` — **all** of them, in array order, ≥2 guaranteed by the schema's `minItems: 2` | — |
| edge cases | `module_brief.failure_story.fail_safe`, then each `module_brief.non_goals[]` in order | — |
| — | `sow_seed.blast_radius` → **not in the text**; carried in `lld_provenance` (§5.3) | depth_evidence.blast_radius → attestation element |
| — | `sow_seed.owner` → checked in step 4, carried in `lld_provenance`; `crew.sow` has no owner slot | owner → SOW owner |
| — | `sow_seed.registry_verdict` → step 7, never text | registry_verdict → intake branch |

`crew.sow` appends its own `" Inferred constraint: preserve existing contract shapes and keep
acceptance checks deterministic."` to every restatement (`sow.py:427`). That is what satisfies 03 §6's
"passes the SOW's not-an-echo bar" — it is mechanical and already shipped; the compiler must not
duplicate or pre-empt it.

### 6.2 Why exactly one leaf

`accepts_when` is a single object, not an array, and `AtomicLeaf.__post_init__` refuses any leaf
without **exactly one** predicate. One `accepts_when` → one leaf. Fanning the freeze out into several
invented leaves would be fabrication; `F07-T10` pins the single leaf to the fixture's own `then` text.

### 6.3 The citation — a disclosed imprecision, not a clean mapping

`crew.sow` refuses a challenge with no evidence, and accepts only a **real `file:line`** or a corpus id
matching `^[A-Z]{1,3}\d+$`. Both are problems here:

- **`file:line` is unusable.** `_default_repo_root` resolves to `Light/fleet`, and `_resolve_file`
  requires the citation to resolve **under that root and to an existing file with ≥ that many lines.**
  An `lld.v1` for an orb module cites orb paths (`complete_freeze.json`'s `acceptance.artifact` is
  `apps/mobile/src/build/lld-v1.ts`) which resolve outside `fleet/` and fail. A `build_new` module's
  artifact does not exist yet **by definition**.
- **The corpus id is a truncation.** Emitting `C1-OPEN` makes `_find_citations`' `\b[A-Z]{1,3}\d+\b`
  extract `C1`, which passes `_CORPUS_ID`. The stored `citation` is therefore **`C1` — a prefix of a
  real gate check id, not a failure-corpus reference.**

**Disclosed, not hidden:** the full id is present in the emitted text where a human reads it; only the
machine's extracted token is truncated. The compiler **must not invent** a citation: if no
`checked_check_ids` entry yields a matching token, **refuse** `NO_CITABLE_EVIDENCE` (§6.5). Fabricating
a plausible `file:line` to get past this check is the exact failure mode `fleet/PRINCIPLES.md` names —
*a check cheaper to fake than to satisfy will be faked* — and is a contract breach, not a shortcut.
*Revive trigger:* widen `_CORPUS_ID` to `^[A-Z]{1,3}\d+(-[A-Z]+)*$` in `crew/crew/sow.py` (a crew-side
change with its own tests — **not** in F07's scope).

### 6.4 The estimate — an honest non-estimate

Nothing in `lld.v1` carries an estimate; `_parse_estimate` only requires the section to be non-empty.
The compiler emits the **exact literal** in the grammar above. Writing `"1 engineer-day"` — or any
number — would be inventing a fact the freeze does not contain. `F07-T12` asserts the literal by exact
string match, so a builder who "improves" it fails the suite.
*Revive trigger:* `lld.v1` gains an `estimate` field via F02's lane and a human-merged contract change.

### 6.5 Sanitization — applied to every interpolated string, in this order

| # | rule | on violation |
|---|---|---|
| 1 | collapse all whitespace runs (incl. `\n`, `\r`, `\t`) to a single space; trim | — (lossless; **required**, the parser is line-oriented) |
| 2 | empty after collapse | refuse `EMPTY_AFTER_NORMALIZE` |
| 3 | matches `(?i)(predicate\|acceptance)\s*[:=]` | refuse `SOW_TEXT_MARKER_COLLISION` |
| 4 | contains `\|` | refuse `SOW_TEXT_PIPE_COLLISION` |
| 5 | contains `[` or `]` — **challenge `RISK` only** | refuse `SOW_TEXT_BRACKET_COLLISION` |

Rules 3–5 exist because `_PREDICATE_MARKER` matches **anywhere** on a line (two markers ⇒
`"expected exactly one"` refusal) and `_parse_alternatives` splits on the first `| tradeoff:`
(an embedded pipe silently steals the trade-off). **Refuse loudly; never mangle.** A silently
corrupted SOW is worse than a refused one — it is a wrong spec at machine speed.

Section headers are unreachable by construction: `_SECTION` needs a whole line to be exactly a keyword
plus a colon, and every interpolated value is emitted mid-line after `Task: ` or `- `, on a single line
guaranteed by rule 1.

`Alt{n}` names are distinct by construction (`_parse_alternatives` casefolds and rejects duplicates)
and colon-free, so `text.partition(":")` always splits at the name boundary even when `option` itself
contains a colon.

### 6.6 Determinism (this is a hard requirement, not a nicety)

`id_for_task` is `blake3(task_text)`. If the text is not byte-stable, the SOW written by `--lld` can
never be found by `fleet run --task`. The compiler must contain **no** timestamp, environment read,
random value, absolute path, or iteration over an unordered map. It indexes JSON objects by key and
walks arrays in order — both deterministic for a given input file. `F07-T3` pins this across two
separate processes.

---

## 7. The contract / acceptance suite (T1 — written before the implementation, red on arrival)

**Files the builder MAY NOT EDIT** (any diff touching them is a contract breach; adding a *new* test
is permitted, changing or deleting one is not):

- `fleet/keel/fleet/tests/f07_sow_intake.rs` — the contract suite
- `fleet/tests/acceptance/sow-lld-intake.sh` — the driven end-to-end against the real binary
- `fleet/contracts/fixtures/lld/**` — F02 §7's rule, carried forward: fixtures may be **added**, never modified

`verify.sh` gains one stage, alongside F02's and F06's:

```sh
stage "sow-lld-intake" required bash "bash unavailable" bash tests/acceptance/sow-lld-intake.sh
```

### 7.1 Fixture I specify here, as LEAD (F02 §7.1 authorship rule)

**`fleet/contracts/fixtures/lld/hash_forged_freeze.json`** — a byte-copy of `complete_freeze.json`
with **exactly one** change: a single hex digit of `freeze.content_hash` flipped. Nothing else. It
must satisfy all three:

1. `lld::validate_lld_v1` → **zero** violations
2. `fleet gate lld-ready` on its `module_brief` → **READY**, exit 0
3. `fleet sow --lld` → **refused**, `FREEZE_HASH_MISMATCH`

Rows 1 and 2 are the red-on-arrival proof that §2.1's hole is real. The builder's freedom is limited
to *which* digit flips; the required behaviour is fixed here, before any implementation exists.
**Append a row to `fleet/contracts/fixtures/lld/AUTHORSHIP.md`** recording it at the same disclosure
level as the F02-era fixtures (lead-specified, builder-instantiated; the F07 builder is not the author
of `lld.rs`'s hasher, so the independence rule's purpose holds).

`F07-T6`/`T7`/`T8`/`T9`/`T14` need wrappers built by *composing* existing fixtures at test runtime
(read JSON, substitute one field, write to a tempdir). That is composition inside the test, not
fixture editing — and it keeps each hostile variant adjacent to the assertion that explains it.

### 7.2 Tests

| id | asserts | kills |
|---|---|---|
| **T1** | `complete_freeze.json` → exit **9**; stderr has `SOW_READY_AWAITING_REVIEW id=<64 hex>`; `state/sows/<id>.json` exists; `sow.leaves.len() >= 1`; `sow.alternatives.len() >= 2` | the happy path never running |
| **T2** | **identity round-trip.** Extract the text between `SOW_LLD_TASK_BEGIN`/`END` from T1; run `fleet sow --task "<that text>"` → **same `id`**, exit 9, **still exactly one** record in `state/sows/`, **not** exit 8 (`EXIT_MISMATCH`) | the two intakes producing SOWs `fleet run --task` can never find. *This is the FEATURES.md acceptance line, made mechanical* |
| **T3** | compile the same fixture in **two separate processes** → byte-identical task text and equal `id` | a timestamp, env read, or map iteration leaking into the text |
| **T4** | `hash_forged_freeze.json` → exit **7**, reason `FREEZE_HASH_MISMATCH`, **no record written**. **Controls, same file:** `fleet contract lld validate` → exit **0**; `fleet gate lld-ready` on its brief → exit **0** | trusting `content_hash`. The two controls are the point — they prove F07 is the only thing catching it |
| **T5** | `forged_freeze.json` (`stamped_by: "orb:lld-ready"`) → non-zero, **before** any file appears in `state/sows/` | ordering the guards after the write |
| **T6** | wrapper whose `module_brief` carries `numeric_trap.json`'s grafted `depth_evidence.score.ratio: 1.0` → exit 7 `FREEZE_UNHASHABLE`, **no panic** | `unwrap()` on `NumericLeafError` |
| **T7** | `registry_verdict.kind == "install"` → exit 7, reason `REGISTRY_REUSE`, `matched_path` echoed, **no record written**. Same for `"extract"`. `"build_new"` proceeds (T1) | 03 §6's short-circuit being documented and not implemented — a duplicate of a registry capability getting built |
| **T8** | `module_brief.registry` ≠ `sow_seed.registry_verdict` → exit 7 `REGISTRY_VERDICT_SPLIT` | two copies of one fact silently disagreeing |
| **T9** | `one_line_freeze.json` wrapped → exit **6**, per-check reasons on stderr, **no record written** | the gate being skipped on the `--lld` path |
| **T10** | `sow.leaves[0].requirement` **contains** the fixture's `accepts_when.predicate.then` substring, and `leaves[0]` has **exactly one** predicate | a stub compiler. `_build_leaves` auto-generates `requirement_present(N)` when it finds no marker, so a lazy compiler passes `crew.sow` while carrying none of the freeze |
| **T11** | for each `i`, `sow.alternatives[i].tradeoff` == whitespace-normalized `killed_alternatives[i].revive_trigger`; count matches exactly | invented alternatives, or dropping all but two |
| **T12** | the `Estimate:` line matches the §6 literal **exactly** | a fabricated number (§6.4) |
| **T13** | **regression.** `sow.rs`'s 5 existing unit tests pass unchanged; `fleet sow --task <good task>` exits 9 with the same record shape; a record created via `--lld` is accepted by `fleet sow accept --id` and `accepted_for_exact_task` returns true | the `--lld` path disturbing the `--task` path or the accept flow |
| **T14** | `failure_story.trigger` containing `\| tradeoff:` → exit 7 `SOW_TEXT_PIPE_COLLISION`; `then` containing `acceptance:` → exit 7 `SOW_TEXT_MARKER_COLLISION`; both **before** any record is written | silent corruption (§6.5) |

**Red-on-arrival is required evidence.** The builder runs the suite on the un-implemented tree first
and pastes the real failure output into the evidence file. A test that was green before the feature
existed was testing nothing.

**Mutation adequacy.** For at least T4, T7, T10 and T12, delete the guard the test targets, show the
named test fails with the expected message, restore, show green, and show `git diff` empty. F06's
lane did this for `MeasuredNothing`; it is the bar.

---

## 8. Exit codes and refusal taxonomy

Existing constants, reused: `EXIT_ENV=3`, `EXIT_INVARIANT=6`, `EXIT_REFUSAL=7`, `EXIT_MISMATCH=8`,
`EXIT_SOW_READY=9`.

| condition | exit | reason token | receipt |
|---|---|---|---|
| unreadable / not JSON | 3 | — | no |
| `validate_lld_v1` violations | 8 | per-path lines on stderr | no |
| `content_hash` ≠ recomputed | 7 | `FREEZE_HASH_MISMATCH` (+ both hashes) | yes |
| brief unhashable (numeric leaf) | 7 | `FREEZE_UNHASHABLE` (+ path) | yes |
| owner disagrees across the three copies | 7 | `OWNER_SPLIT` | yes |
| registry verdict disagrees brief vs seed | 7 | `REGISTRY_VERDICT_SPLIT` | yes |
| gate `NotReady` | **6** | per-check reasons | yes |
| gate `MeasuredNothing` | **6** | `MEASURED_NOTHING` | yes |
| `registry_verdict.kind` ∈ {`install`,`extract`} | 7 | `REGISTRY_REUSE` (+ `kind`, `matched_path`, `node_id`) | yes |
| sanitization / citation failures | 7 | `SOW_TEXT_MARKER_COLLISION`, `SOW_TEXT_PIPE_COLLISION`, `SOW_TEXT_BRACKET_COLLISION`, `EMPTY_AFTER_NORMALIZE`, `NO_CITABLE_EVIDENCE` | yes |
| `crew.sow` refuses the compiled text | 7 | `SOW_AMBIGUOUS` — **existing path, unchanged** | yes (existing) |
| **success** | **9** | `SOW_READY_AWAITING_REVIEW` — **existing path, unchanged** | yes (existing) |

**Why gate failures exit 6, not 7:** `print_gate_verdict` already returns `EXIT_INVARIANT` for both
`NotReady` and `MeasuredNothing`. The same condition must not report two different codes depending on
which command the operator typed. Consistency with the shipped gate beats taxonomic tidiness.

**Ordering is load-bearing.** Every guard runs before `create_sow`, so **no refusal path ever leaves a
`state/sows/` file behind.** T5, T7, T9 and T14 each assert the absence, not just the exit code.

---

## 9. Files owned by this lane (no overlap with any other lane)

| file | action |
|---|---|
| `fleet/keel/fleet/src/sow.rs` | **extend** — `compile_task_text`, `CompileError`, `ready_record_with_provenance`. Existing fns and tests untouched |
| `fleet/keel/fleet/src/main.rs` | **extend** — one `sow_command` arm, `create_sow_from_lld`, one usage line. `create_sow`/`accept_sow`/`enforce_accepted_sow` untouched |
| `fleet/keel/fleet/tests/f07_sow_intake.rs` | **new** (contract suite — LEAD-owned, builder read-only) |
| `fleet/tests/acceptance/sow-lld-intake.sh` | **new** (contract suite — LEAD-owned, builder read-only) |
| `fleet/contracts/fixtures/lld/hash_forged_freeze.json` | **new** (instantiate per §7.1) |
| `fleet/contracts/fixtures/lld/AUTHORSHIP.md` | **append one row** |
| `fleet/verify.sh` | **append one stage** |
| `docs/lane-contracts/F07-sow-intake.md` | this file (do not edit) |

Not touched: `lld.rs`, `lld_ready.rs`, `crew/**`, any contract schema, any other lane's files.

---

## 10. Done-definition

1. Worktree fast-forwarded onto `master`; `lld_ready.rs` present (§0).
2. All 14 tests in `f07_sow_intake.rs` green; `sow-lld-intake.sh` green.
3. **Red-on-arrival output pasted** into the evidence file for the whole suite.
4. **Mutation evidence** for T4, T7, T10, T12 (delete guard → named test fails with the expected
   message → restore → green → `git diff` empty).
5. **Driven manually, on the real binary, not only in-process:** `fleet sow --lld
   contracts/fixtures/lld/complete_freeze.json` → exit 9; the printed task text pasted verbatim into
   `fleet sow --task "<it>"` → same id, one record; `fleet sow accept --id <it>` → accepted;
   `fleet run --task "<it>"` gets **past** the SOW gate (it may fail later for unrelated reasons —
   record what actually happened, do not claim more).
6. `bash verify.sh` run **foreground, unpiped** (`$?` read directly — not `tee`'s), full
   passed/failed/skipped counts reported including pre-existing reds, each failure traced to a
   file:line and shown disjoint from this lane's files.
7. `git diff --stat` shows **zero** changes under `fleet/contracts/fixtures/lld/*.json` except the one
   added file, and **zero** changes to `f07_sow_intake.rs` / `sow-lld-intake.sh` after they were written.
8. Evidence file written; `status/F07-*.status` updated; dated `FLEET-LEARNINGS.md` entry appended to
   the **root** file (`/Users/rachitsrivastava/youtube/Principal Engineering/FLEET-LEARNINGS.md`).
9. **Ship to a PR and stop.** This lane touches `verify.sh` and adds a contract-adjacent fixture —
   **human-merge always** (A15/D4). Do not merge.

---

## 11. Explicitly out of scope

- **F09 — orb→fleet wiring.** F07 accepts an `lld.v1` **file**, however it arrives. No WebSocket, no
  relay, no freeze-event listener, no transport. If a design choice here would only pay off once F09
  exists, it is F09's choice, not this lane's.
- **The Reviewed-edge short-circuit** (04 §1) — killed as K3. Exit 9 and the human accept are unchanged.
- **Adding `freeze_receipt_hash` / `reviewed_by` / `reviewed_at`** to any schema — F02's lane, human-merge.
- **Adding `estimate`** to `lld.v1` — F02's lane (§6.4's revive trigger).
- **Widening `_CORPUS_ID`** in `crew/crew/sow.py` — crew's own change (§6.3's revive trigger).
- **Recomputing `blast_radius`** from the code graph — F06 §12 defers it until `graph.rs` exposes a
  pure `impact()` **and** `registry/` is non-empty. F07 carries the proposer's string forward and says
  so.
- **The 9-element attestation, `subject.digest`, the PR emit** — F08 / S9–S10 (§2.2 only fixes the
  algorithm-name hazard so a later lane inherits a correct value).
- **The S1–S10 lane walk, blind-suite compilation into REQ-ID predicates, model routing** — 04 §§3–6,
  F13/F24.
- **Gate calibration / threshold tuning** — F17.
- **A live registry scan** replacing `gate-refs.v1.json`'s checked-in list — F25.
