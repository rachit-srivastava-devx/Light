# L8 Final Comprehensive Review — 2026-09-12

Reviewer: fleet lead (Opus) · Scope: all 32 `docs/blueprints-next/*/BLUEPRINT.md`
Method: section-scoped parse of all 32 files on the 4 mechanical axes + the 18-point spec items,
then adversarial cross-section reads (§4 ↔ §9 ↔ §10 ↔ §11 ↔ §13 consistency).
Every verdict below is a counted file set with a quoted line, not an impression.

## OVERALL VERDICT: PARTIAL
## Blocking defects: 3

Structural baseline: 32/32 carry all 14 sections. §9 rewrite landed (31/32). §10 named-test tables
landed (32/32 carry 3 exact test IDs). The §11 "Function mutated" wave landed the *column header*
everywhere but bound the rows to a file that §4 actually declares in only 10/32.

---

## 4-Axis Gate Results (all 32 nodes)

| Node | §9 file paths | §13 bullets | §10 named table | §11 Func mutated |
|---|---|---|---|---|
| approval | PASS | PASS | PASS | PARTIAL |
| broker | PASS | PASS | PASS | FAIL |
| builder | PASS | PASS | PASS | FAIL |
| candidate | PASS | PASS | PASS | PASS |
| connectors | PASS | PASS | PASS | PASS |
| context | PASS | PASS | PASS | PASS |
| control | PASS | PASS | PASS | FAIL |
| dag | PASS | PASS | PASS | PARTIAL |
| ingest | PASS | PASS | PASS | PARTIAL |
| integrate | PASS | PASS | PASS | PASS |
| intent | PASS | PASS | PASS | PARTIAL |
| knowledge | PASS | PASS | PASS | PARTIAL |
| model_catalog | PASS | PASS | PASS | PARTIAL |
| next_plan | PASS | PARTIAL | PASS | FAIL |
| notify | PASS | PARTIAL | PASS | FAIL |
| offline | PASS | PASS | PASS | PASS |
| plan_review | PASS | PARTIAL | PASS | PASS |
| planner | PASS | PASS | PASS | PASS |
| post | PASS | PASS | PASS | PARTIAL |
| probe_business | PASS | PASS | PASS | PASS |
| probe_learn | PASS | PARTIAL | PARTIAL | PARTIAL |
| probe_research | PASS | PARTIAL | PARTIAL | PARTIAL |
| probe_tech | PASS | PARTIAL | PARTIAL | PARTIAL |
| questions | PASS | PARTIAL | PASS | PASS |
| ready | PASS | PASS | PASS | PARTIAL |
| review | PASS | PASS | PASS | FAIL |
| rollback | PASS | PASS | PASS | FAIL |
| route | PASS | PASS | PASS | FAIL |
| scan | PASS | PASS | PASS | PARTIAL |
| store | PASS | PASS | PASS | PARTIAL |
| user_cli | PASS | PASS | PASS | PASS |
| verify | **FAIL** | PARTIAL | PASS | **FAIL** |

Totals — §9: 31 PASS / 1 FAIL · §13: 24 PASS / 8 PARTIAL · §10: 29 PASS / 3 PARTIAL ·
§11: 10 PASS / 13 PARTIAL / 9 FAIL.

### Axis grading rules applied
- **Axis 1 PASS** = ≥3 of 5 steps carry a backticked `src/…rs` / `tests/…rs` path **and** ≥3 end in a
  `cargo test|check -p <crate> …` invocation.
- **Axis 2 PARTIAL** = itemized and checkable, but the awk line-cap gate is absent from §13 (it is
  present in §12 for all 8). No §13 is prose — the point-2 FAIL from the prior review is cleared.
- **Axis 3 PARTIAL** = 3 exact test IDs present but the heading is not `###` (`probe_learn`,
  `probe_tech` use bold) or the block is `####` prose subsections rather than a 4-column table
  (`probe_research`). Form deviation only; content is complete.
- **Axis 4 PASS** = header contains `Function mutated` **and every row** names `` `fn` `` in a
  `src/<file>.rs` **that §4 declares**. PARTIAL = some rows qualify. FAIL = no row does, or the
  header is mislabeled.

---

## 18-Point Spec Compliance

| Point | Status | Notes |
|---|---|---|
| 4 (4B-implementable steps) | **PARTIAL** | 31/32 PASS. `verify/BLUEPRINT.md:114-118` is still the pre-wave prose form — zero file paths, zero cargo commands. |
| 7 (mutation floors) | **PARTIAL** | All 11 authority nodes state ≥80% and all others ≥75% somewhere. But `intent` (authority) states **75%** in §11 and **80%** in §12/§13 — a builder reading §11 uses the wrong floor. `builder`, `context`, `questions` carry the same 75-vs-80 contradiction (non-blocking: both ≥ their floor). |
| 12 (library reuse / pinned Cargo) | **PARTIAL** | 28/32 carry `### Cargo.toml snippet (pinned)` in §7 with the correct `serde = { version = "1", features = ["derive"] }` form. 3 have the block **misfiled at the end of §11**: `candidate:112`, `context:113`, `scan:120`. `review` pins correctly but only inside the §7 reuse table — no snippet block. No node uses bare `serde = "1"`. |
| 13 (crate separation) | **PASS** | 32/32 declare `crates/<node>/` in §4 with per-file ≤N-line caps. Kebab-case dirs with snake_case test paths (`crates/next-plan/` → `next_plan::tests::`) is correct Rust convention, not a defect. `scan` correctly declares edit-in-place of the existing `crates/fleet-scan/`. No monolith. |
| 15 (low-parameter agent readiness) | **PARTIAL** | §5 is elision-free 32/32 and §9 is self-contained 31/32. Broken by the §9↔§11 file contradiction in 8 nodes (below): an agent that builds the files §9 names cannot locate the functions §11 tells it to mutate. |
| 18 (LLD mirror) | **PASS** | 32/32 carry the exact `docs/LLD/lld-full-detail.architecture.json:components[id=<node>]` form in §1, colon included, and every id matches its directory. The 9 malformed paths from the prior review are fixed. |

---

## Remaining defects

### BLOCKING 1 — `verify` §9 never received the file-path rewrite
`docs/blueprints-next/verify/BLUEPRINT.md:113-118`:
> `1. Define gate/report/error records → compile.`
> `2. Port existing `fleet_verify::run_all` → parser/count tests.`
> `3. Add real binary runner and protected acceptance → integration test.`

Zero of 5 steps name a file; zero end in a cargo command. This is the one node the prior review's
defect #1 was never applied to — and it is an authority node whose §10 names three tests
(`verify::tests::reviewed_candidate_runs_deterministic_gates`, `…failing_gate_produces_gate_evidence_not_panic`,
`…coverage_below_floor_marks_gate_failed`) that no §9 step creates. Fix: bind the 5 steps to the
`src/runner.rs` / `src/receipt.rs` files §4 already declares, in the `approval/BLUEPRINT.md:79-83` form.

### BLOCKING 2 — §9 and §11 name *different files* for the same function (8 nodes)
The Axis-4 wave populated the `Function mutated` column with plausible `fn` in `src/file.rs` values
without reconciling against §4's declared layout. The named file does not exist in 8 nodes:

| Node | §4 declares | §11 points at | Contradiction |
|---|---|---|---|
| `control` | `src/reducer.rs` | `src/state.rs` (`:124`), `src/intent.rs` (`:125`) | §9 step 2 says "In `src/reducer.rs`: … pure `reduce` function"; §11:124 says `` `reduce` in `src/state.rs` `` — **0/5 rows bound** |
| `broker` | `src/effect.rs`, `authorize.rs`, `provider.rs`, `reconcile.rs` | `src/dispatch.rs` (`:113`), `src/receipt.rs` | **0/5 rows bound** |
| `builder` | `src/lease.rs`, `launch.rs`, `result.rs` | `src/scope.rs` (`:103`), `src/frame.rs` (`:104`) | **0/4 rows bound** |
| `intent` | `src/schema.rs`, `prompt.rs`, `gate.rs` | `src/parse.rs` | §9 step 2 puts `validate` in `src/schema.rs`; §11 says `src/parse.rs` — 1/4 bound |
| `model_catalog` | `src/discover.rs`, `probe.rs`, `qualification.rs` | `src/catalog.rs` | 1/4 bound |
| `post` | `src/verify.rs`, `receipt.rs`, `port.rs` | `src/publish.rs` (`:104`) | §9 step 3 puts `GateRunner` in `src/port.rs` — 3/5 bound |
| `approval` | `src/grant.rs`, `check.rs`, `port.rs` | `src/consume.rs` (`:111`, `:114`) | 3/5 bound |
| `ingest` | `src/envelope.rs`, `registry.rs`, `dedup.rs` | `src/validate.rs` (`:125`) | 3/4 bound |

Why this is blocking and not cosmetic: the §12 recipe points `cargo-mutants` at the crate, and §11 is
the reviewer's checklist of *which function to mutate*. Pointed at a file that was never created, the
mutation step finds nothing to mutate and reports a vacuous pass. That is the
`PRINCIPLES.md` failure mode verbatim — a check cheaper to fake than to satisfy.

### BLOCKING 3 — §11 `Function mutated` column holds no function in 9 nodes
Header present, column contents are behaviors or test names:
- `rollback/BLUEPRINT.md:113` — `| containment check in `src/guard.rs` | …` (all 6 rows are behaviors; **0/6** name a function)
- `route/BLUEPRINT.md:121` — `| capability filter in `src/filter.rs` | …` (**0/4**)
- `review/BLUEPRINT.md:156` — `| `run_semgrep` | …` (names functions but **no file** on any row; last 2 rows are prose: "omit acceptance digest", "allow worker self-review")
- `next_plan`, `notify` — functions named, **no file path on any row**; trailing rows are `(queue capacity)`, `(cursor)`, `(idempotency key)`
- `verify/BLUEPRINT.md` §11 — column is actively **mislabeled**: header is `| Mutant | Function mutated | Killing test | Proof |` and 5 of 8 rows put a *test* in the function column (`| skip one gate | denominator/hidden test | …`). Only rows 6-8 name real functions, and their file binding sits in a separate bullet list below the table rather than in the row.
- `broker`, `builder`, `control` — counted here and in BLOCKING 2.

`cargo-mutants` cannot be aimed at "containment check" or "denominator/hidden test".

### Non-blocking
- **awk gate absent from §13** in 8 nodes (`next_plan`, `notify`, `plan_review`, `probe_learn`,
  `probe_research`, `probe_tech`, `questions`, and form-variant `verify`) — replaced by prose,
  e.g. `notify` §13: "No source file under `crates/notify/src/` exceeds 80 lines (check with `wc -l`)".
  Not runnable as an exit-code assertion. The gate is present in each node's §12.
- **`review` §12 has no awk gate** (it is in §13 row 6 instead) — the mirror of the above.
- **`probe_learn`, `probe_research`, `probe_tech` §13 carry no numeric `caught/total` floor** — only
  per-mutation manual instructions.
- **`probe_tech` §13 names its tests via `grep -E "fn (…)"`** rather than a `probe_tech::tests::…` ID.
- **`scan` §7 snippet declares `fleet-scan = { path = "../../crates/fleet-scan" }`** — a self-dependency
  in the crate's own manifest (`scan/BLUEPRINT.md:120-126`). Wrong for an edit-in-place node.
- **`verify` §13 uses a fenced `[ ]` checklist rather than markdown bullets** — substance is complete
  (cargo test, 3 named IDs, clippy, awk gate, `cargo mutants … >= 0.80`); form only.
- **§13 references tests absent from §10's named table**: `planner` (`rejects_empty_acceptance`),
  `post` (`all_gates_required`), `store` (`incomplete_blob_is_rejected`), `rollback`
  (`rollback_emits_requeue_edge_on_shared_ref`).
- **`approval` §13:137** says "the 3 named mutation targets in §11" but §11 lists 5 rows.
- **`approval` §10 names `expired_grant_refused`** which no §9 step creates (§9 step 3 creates only
  `replay_refused_after_consume`). Same shape in `broker`, `builder`, `context`, `control`,
  `probe_business`.

### Verified fixed since the prior review (no action)
Point 2 (§13 prose in 20/32) — now 0/32 prose. Point 18 (9 malformed LLD paths) — now 32/32 exact.
Point 3 (§10 unnamed in 21/32) — now 32/32 carry 3 exact IDs. `probe_business` §12 awk gate — present.
§7 rejection line, Cargo snippets for `user_cli`/`review` reuse table — present.
`scan` §9 is now the strongest in the tree (fully-qualified `cargo test -p fleet-scan fleet_scan::tests::<name>`).

---

## Conclusion

Three of the four mechanical axes are effectively closed: §9 is builder-executable in 31/32, §13 is
itemized and command-backed in 32/32, and §10 carries three exact test IDs in 32/32 — the three
blocking defects from the prior review are genuinely fixed, not papered over. The tree is much closer
to parallel-build-safe than it was. What is not closed is Axis 4: the `Function mutated` column was
added everywhere but is only trustworthy in 10 of 32 nodes, and in 8 nodes it names `src/*.rs` files
that §4 never declares and §9 never creates — `control`, `broker` and `builder` have **zero** rows that
resolve to a real file. Combined with `verify` §9 never receiving the rewrite at all, this means the
anti-stub layer — the one layer whose whole job is to prove the implementation is not a stub — is
itself the least verified part of the tree, and would report vacuous success if run as written. I am
holding this at **PARTIAL**. Fix order: (1) `verify` §9, one file, mechanical; (2) reconcile §11 file
paths against §4 in the 8 nodes, mechanical and derivable from §4; (3) replace behavior descriptions
with real function names in the 9 nodes, which needs the §5 API contract as input. The non-blocking
items (awk gate placement, the three `probe_*` form deviations, the `scan` self-dependency) can ride
along. Contracts are human-merge under A15/D4 — this goes to a PR, not to main.
