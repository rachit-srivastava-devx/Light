# L8 comprehensive review — `docs/blueprints-next/` (32 nodes)

Date: 2026-09-12 · Reviewer: fleet lead (Opus) · Method: mechanical grep over all 32 `*/BLUEPRINT.md`,
section-scoped reads on the 12-node sample (approval, broker, control, dag, ingest, intent, knowledge,
notify, post, rollback, route, user_cli). Every finding below is a counted file set, not an impression.

Structural baseline: **32/32 nodes carry all 14 sections `## 1.`–`## 14.`** — no node is skeletal.

---

## 1. Quality evaluation criteria (§10 test matrix + §14 failure stories) — **PASS**
§10 and §14 present in 32/32. §14 is uniform: exactly one `**Failure → Cause → Fix:**` triple plus 3 review
questions per node. Sample: `approval/BLUEPRINT.md:125`, `notify/BLUEPRINT.md:152`, `route/BLUEPRINT.md:141`.
Caveat (non-blocking): **n=1 failure story per node, 32/32** — no node carries a second story, so the section is
structurally satisfied but at minimum depth.

## 2. Definition of done — checkable assertions with denominators — **FAIL (blocking)**
**20/32 §13 sections are a single unbulleted prose sentence** averaging ~46 words, with 0 commands and 0 denominators:
approval, broker, builder, candidate, context, control, ingest, integrate, intent, model_catalog, planner, post,
probe_business, ready, review, rollback, route, store, user_cli, verify.
Defect quote — `route/BLUEPRINT.md:137`:
> `Done means route is pure, deterministic, conservative, integer-scored, snapshot-bound, fully explained, and integrated with intent/scan/control tests; zero-input and stale snapshots fail; no provider availability is inferred from a manifest.`

"fully explained", "conservative", "pure" are unfalsifiable. `approval/BLUEPRINT.md:122` is the same shape.
The working reference exists in-tree — `notify/BLUEPRINT.md:137-150` is 10 bulleted assertions, each a command or a
named test with expected output (`cargo clippy -p notify --all-targets -- -D warnings` exits 0; output contains
`checked=1,total=1`). 12/32 reach that bar (dag, knowledge, next_plan, notify, offline, plan_review, probe_learn,
probe_research, probe_tech, questions, scan + partial). **Fix: port the notify §13 form to the other 20.**

## 3. Complete test categories; integration tests named — **PARTIAL**
All five categories (unit / integration / hidden / property / differential) present in **32/32** §10 — verified by
keyword count per section. *Named* integration tests only in **11/32**: next_plan, notify, offline, plan_review,
probe_business, probe_learn, probe_research, probe_tech, questions, review, verify.
The other **21/32 describe integration tests in prose with no test identifier** — `approval/BLUEPRINT.md:88`:
> `**Integration/contract tests:** passing post verdict creates a broker input only after grant consumption; notify receives state change.`
vs the correct form at `notify/BLUEPRINT.md:103`: `` `notify::tests::state_change_emits_redacted_notification` `` with
an Inputs / Expected output / Mutation caught table.

## 4. §9 numbered tiny steps with specific file names — **FAIL (blocking)**
32/32 have exactly 5 numbered steps. **31/32 contain ZERO `src/*.rs` file paths in §9** (only `questions` has 1).
`approval/BLUEPRINT.md:78-84` in full:
> `2. Port HumanApproval evidence without treating it as the grant → lifecycle parity test.`
> `4. Add durable store and receipt transaction → restart test.`
§4 names the files (`src/grant.rs`, `src/check.rs`, `src/port.rs` with line caps) but §9 never binds a step to one.
The builder must infer the step→file mapping.

## 5. Anti-stub evidence — §11 names the test each mutation kills — **PARTIAL**
**22/32 §11 name tests only as prose descriptions.** `approval/BLUEPRINT.md:100-105`: mutant `ignore scope hash` →
test `scope mismatch test` — not a runnable identifier. Only 10 name qualified tests: next_plan, notify,
probe_research, probe_tech, questions, review, verify (+3 partial). Correct form at `verify/BLUEPRINT.md` §11:
`` `verify::tests::coverage_below_floor_marks_gate_failed` `` with the failing assertion spelled out
(`assert!(!gate.passed)`).

## 6. Mutation floor line + executable awk gate — **PARTIAL**
A mutation-floor line exists in **32/32**, but 6 omit the `caught/total` ratio and state a bare percentage:
dag (`mutation floor ≥80%`), knowledge, planner (`mutation floor is met`), questions, ready, scan.
awk gate `find crates/<n> -name '*.rs' -exec awk '{c[FILENAME]++} …'` present in **31/32** —
**`probe_business` §12 has no awk gate**, only the comment `# Mutation floor: caught/total >= 75%`
(`probe_business/BLUEPRINT.md` §12), so its ≤80-line claim is unenforced.
§12 denominators (`checked=N,total=N` or `N/N`) present in 30/32 — missing in **context, next_plan**.
`cargo clippy … -D warnings` present 32/32.

## 7. 4B-model step atomicity — **FAIL (blocking)**
Same root cause as #4. Steps are verb-abstractions with a test gesture, not executable instructions.
`control/BLUEPRINT.md:94`: `Add transaction-before-launch supervisor → killed-controller recovery test.`
`dag/BLUEPRINT.md:73`: `Run graph property, clippy, and a real `fleet` dry-run once composition exists.`
A model with no repository context cannot open a file from these.

## 8. Why we build it (§1) + what it does NOT own (§2) — **PASS**
`- Why this node exists:` (or `- Why:`) present in 32/32 §1; `**Does not own:**` present in 32/32 §2, 4–7 items each.
Evidence: `approval/BLUEPRINT.md:6,15`, `dag/BLUEPRINT.md:6,14`, `route/BLUEPRINT.md:7,21`.

## 9. Library research — pinned Cargo.toml snippet — **PARTIAL**
`### Cargo.toml snippet (pinned)` (or an exact-snippet variant, e.g. `probe_research`, `review`) in **27/32**.
**5 nodes have no Cargo.toml block anywhere in the file and no `serde`/`thiserror` pin: candidate, context, scan,
store, user_cli.** `user_cli/BLUEPRINT.md` §7 defers to "version pinned by workspace lock" — that is a pointer,
not a pin a builder can paste. Note also that the 27 pin to semver ranges (`serde = { version = "1" … }`,
`thiserror = "2"`), not exact versions; only third-party crates are exact (`petgraph = "0.8.3"`,
`route/BLUEPRINT.md` §7).

## 10. Parallel build readiness — no cross-blueprint internal references — **PASS**
Zero references to `blueprints-next/<other>/`, to another `BLUEPRINT.md`, or to "§N of <node>" across all 32.
Inter-node coupling is expressed only as named edges in §1 (`approval -> broker`) and shared types in §5 — the
correct seam. The `crates/fleet-*/src/*.rs` citations found in 29/32 files are §7 reuse pointers into the *existing*
codebase, not into a sibling blueprint's proposed internals.

## 11. Different-model re-verification — **SKIPPED** (not checkable in blueprints, per brief).

## 12. Library reuse rule (reuse table AND Cargo snippet) — **PARTIAL**
Reuse table present 32/32 (4–12 rows; columns: source / evidence+license / use / why not custom / proof still
required). Fails only on the snippet half — the same 5 nodes as #9. No instance found of a blueprint specifying
from-scratch work that a listed crate already provides.

## 13. Crate separation — §4 path, files, line limits — **PASS**
32/32 name `crates/<name>/`, the specific `.rs` files, and a per-file line cap. Two encodings, both complete:
fenced tree (`approval/BLUEPRINT.md:20-28`, `src/check.rs # exact-match/expiry/replay, ≤80 lines`) and one-line
prose (`dag/BLUEPRINT.md:19`, `src/graph.rs` owns validation/cycles (≤80) … All live under `crates/dag/`).

## 14. Existing-code decisions — reuse seams + explicit rejection — **PARTIAL**
Reuse seams cited with `file:line` ranges in 32/32 (e.g. `crates/fleet-lifecycle/src/human_approval.rs:10-25`,
`approval/BLUEPRINT.md` §7). Explicit non-adoption sentence in **27/32** (`Petgraph is not route authority; current
router logic remains Fleet-owned.` — `route/BLUEPRINT.md` §7).
**5 nodes have §7 as table + Cargo block only, with no rejection statement: builder, context, control, planner, ready.**
`control/BLUEPRINT.md` §7 contains no prose line at all.

## 15. Low-parameter agent readiness — **PARTIAL**
§5 half **PASSES cleanly**: 32/32 have a ```` ```rust ```` fence, and **0/32 contain any elision** (`...`, `…`, `TBD`,
`etc.`). Signatures are fully typed — `approval/BLUEPRINT.md:33`:
`pub trait ApprovalStore { fn put(&mut self, grant: &ApprovalGrant) -> Result<(), ApprovalError>; fn consume(&mut self, id: &str) -> Result<ApprovalGrant, ApprovalError>; }`
§9 half **FAILS** — see #4 (31/32 with no file names).

## 16. Test correctness — tests that fail on a stub — **PASS**
32/32 §10 carry explicit stub-defeating language. Strongest form at `notify/BLUEPRINT.md:103-107`, a Mutation-caught
column per test ("Any impl that calls `unwrap()` on the transport result fails"). Weaker but present elsewhere —
hidden/property/differential rows are stated so a pass-through implementation cannot satisfy them.

## 17. Responsibility — §11 named mutation targets (function names) — **PARTIAL**
**Only 2/32 name functions with their file: `review` and `verify`** (`verify/BLUEPRINT.md` §11: `run_gate` in
`src/runner.rs`; `evaluate_coverage` in `src/runner.rs`; `assemble_gate_evidence` in `src/receipt.rs`).
13 more name a bare function identifier (`redact_pii`, `emit_next_plan_signal`). The remaining ~17 name only
behaviors — `approval/BLUEPRINT.md:101`: mutant `ignore scope hash`, which is a description, not a target
`cargo-mutants` can be pointed at.

## 18. LLD path mirroring with the colon — **PARTIAL**
Exact form `lld-full-detail.architecture.json:components[id=<node>]` in **23/32**; all 23 ids match their directory.
**9 deviate** — control, ingest, model_catalog use a space instead of the colon
(`…architecture.json components[id=control]`, `control/BLUEPRINT.md:6`); and **6 drop the filename entirely** —
dag, probe_business, probe_learn, probe_research, probe_tech, questions all read
`architecture JSON \`components[id=dag]\`` (`dag/BLUEPRINT.md:5`), leaving a context-free agent no path to resolve.

---

## Score

| Verdict | Points |
|---|---|
| PASS | 1, 8, 10, 13, 16 (5) |
| PARTIAL | 3, 5, 6, 9, 12, 14, 15, 17, 18 (9) |
| FAIL (blocking) | 2, 4, 7 (3) |
| Skipped | 11 |

## Blocking defects, in fix order

1. **§9 has no file names (31/32)** — points 4, 7, half of 15. Bind each of the 5 steps to the `src/*.rs` file §4
   already names. Mechanical; the mapping is derivable from §4 in every file.
2. **§13 is prose in 20/32** — point 2. Port the `notify/BLUEPRINT.md:137-150` bulleted form; each item a command
   plus expected output or a named test.
3. **§10 integration tests unnamed in 21/32 and §11 mutation targets unnamed in ~17/32** — points 3, 5, 17.
   Adopt the `notify`/`verify` table form: test identifier, inputs, expected output, mutation caught.

Non-blocking, cheap: `probe_business` §12 missing awk gate; 5 nodes missing Cargo snippet (candidate, context,
scan, store, user_cli); 5 nodes missing §7 rejection line (builder, context, control, planner, ready); 9 nodes with
a malformed LLD path; 6 floor lines missing `caught/total`; `context`/`next_plan` §12 missing denominators.

## OVERALL VERDICT: PARTIAL
A 4B model can create the crate, paste §5's signatures, and paste §7's Cargo.toml from 27 of 32 blueprints — but it cannot execute §9 in 31 of 32 (no step names a file) and cannot tell it is finished in 20 of 32 (§13 is adjectives, not assertions), so parallel autonomous build is not yet safe to launch.
