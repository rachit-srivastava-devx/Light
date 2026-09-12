# L8 TARGETED RE-CHECK — 2026-09-12

Scope: re-check of the 3 highest-priority findings from `L8-COMPREHENSIVE-2026-09-12.md`.
Sample (as briefed): `approval broker control ingest integrate route store user_cli`.
Full-corpus counts (32 nodes) added where the sample turned out to be unrepresentative.

Method: mechanical extraction per section, not reading impressions.

```
§9  pass = >=3 of 5 steps match  ^N. In `(src|tests)/<file>.rs`
§13 pass = >=2 bullets containing a `cargo ` command
§10 pass = >=1 qualified identifier matching  <crate>::tests::<name>
§11 pass = mutation table has a "Function mutated" column
```

## Area 1: §9 file paths

**PASS — 8/8 sampled have file names** (all 8 have file paths in **5/5** steps, not just 3).

Quote from `approval` §9 step 2:

> 2. In `src/check.rs`: port HumanApproval evidence without treating it as the grant → add `approval::tests::grant_consumed_on_approval` and run `cargo test -p approval grant_consumed_on_approval` exits 0.

Quote from `route` §9 step 2:

> 2. In `src/filter.rs`: Implement capability, policy, and quota filters as named pure predicate functions; `cargo test -p route capability_filter_rejects_missing` passes.

Both now carry file + named test + runnable command. This is the intended form.

**Full corpus: 19/32.** The sample is not representative — it drew entirely from the fixed cohort.
13 nodes still have **0/5** steps with a file path: `connectors dag knowledge next_plan notify offline
plan_review probe_learn probe_research probe_tech questions scan verify`. Their §9 is still the old
prose form, e.g. `notify` step 1: "Define effect/grant/receipt/transport types → compile."

## Area 2: §13 checkable assertions

**PASS — 8/8 sampled have bullets with cargo commands**, named test identifiers, and denominators.

Quote 2 bullets from `control` §13:

> - `control::tests::ingest_event_drives_state_then_intent` appears in test output and passes.
> - Mutation floor: `caught/total >= 80%` for the 3 named mutation targets in §11 (launch-before-commit, stale-generation, infinite-retry).

Quote 2 bullets from `route` §13:

> - `route::tests::refusal_receipt_contains_first_empty_stage` appears in test output and passes.
> - `cargo clippy -p route --all-targets -- -D warnings` exits 0 (zero warnings).

**Full corpus: 26/32.** Still failing: `connectors dag knowledge review scan verify`.

Residual gap inside the passing 8 (not blocking, worth one wave): `route` and `store` §13 carry a
`caught/total` denominator but **no `checked=N,total=N` real-binary bullet**, while the other six do.
`route` has `cargo run --bin fleet -- route --json` in §12 but never promotes it into §13, so the
done-definition can be satisfied without ever running the binary. That is a proxy-not-property hole
(PRINCIPLES: an API 200 is not a rendered page).

## Area 3: §10/§11 identifiers

**§10: 0/8 sampled have qualified test identifiers.**
**§11: 0/8 sampled have a "Function mutated" column with file paths.**

All 8 sampled §10 sections are still the prose form:

> **Unit tests:** expiry, scope mismatch, base mismatch, revoke, duplicate, one-use consume.  (`approval`)
> **Unit tests:** each filter, conservative profile floor, tie-break, overflow, unknown evidence, empty set.  (`route`)

Partial credit: `ingest` and `user_cli` name **bare** test identifiers (`unknown_source_refuses`,
`parse_rejects_empty_submit`) but not `<crate>::tests::<name>`. The other 6 are prose-only.

All 8 sampled §11 tables are the 3-column form
`| Mutant or fake implementation | Test that must fail | Why this proves behavior |`
with prose in every cell — no function name, no `src/<file>.rs`, and the "test that must fail" column
names a description ("scope mismatch test") rather than an identifier.

**Prose-only count for the sample: §10 6/8 prose-only (8/8 unqualified); §11 8/8 prose-only.**

### The finding the sample hides

The two fix waves hit **disjoint** cohorts and are near-perfectly anti-correlated.
Full corpus: §10 qualified = 11/32, §11 "Function mutated" = 4/32 — and 9 of those 11 §10-passing
nodes are in the 13 that **fail** §9.

| Cohort | Nodes | §9 | §13 | §10 | §11 |
|---|---|---|---|---|---|
| A (sampled 8, + builder candidate context intent model_catalog planner post ready rollback) | 19 | PASS | PASS | FAIL | FAIL |
| B (`next_plan notify offline plan_review probe_* questions review verify`) | 11 | mostly FAIL | PASS | PASS | 4 PASS |
| C (`connectors dag knowledge scan`) | 4 | FAIL | FAIL | FAIL | FAIL |

**0 of 32 nodes pass all four criteria.** `notify` and `next_plan` come closest (3/4, missing §9).

The good news: both target forms now exist in-tree and can be copied rather than invented.
`notify` §10/§11 is the reference for Area 3 — a 4-column integration table with
`| Test name | Inputs | Expected output | Mutation caught |` and rows keyed on
`notify::tests::sensitive_fields_not_in_notification`, plus a §11 header
`| Function mutated | Mutant or fake implementation | Test that must fail | Why this proves behavior |`.
`approval`/`route` §9+§13 is the reference for Areas 1–2.

### Incidental defect (template drift)

`store` and `user_cli` have a stray `### Cargo.toml snippet (pinned)` block nested **inside §11**
(both at line 119) instead of §7, where `approval` (line 59) and `route` (line 65) correctly place it.
`store` §11 also lost its `Safety mutation floor:` line as a result. Mechanical fix, junior-tier.

## Overall: what remains blocking

Areas 1 and 2 are genuinely fixed for the sampled 8 but only for ~60–80% of the corpus, and Area 3
is not fixed at all in the sample — the real blocker is that three fix waves each patched a different
subset, so **no node is yet complete on all four axes** and the remaining work is a cross-product
(§9+§13 for cohort B, §10+§11 for cohort A, all four for cohort C) rather than the one more pass the
per-area pass rates suggest.

### Recommended next waves (routing per A17)

| Wave | Work | Nodes | Tier |
|---|---|---|---|
| 1 | §10/§11 → identifier form, copying `notify` as the template | 19 cohort-A nodes | junior (mechanical, template exists) |
| 2 | §9 → `In \`src/<file>.rs\`` form, copying `approval` as the template | 13 §9-failing nodes | junior |
| 3 | cohort C (`connectors dag knowledge scan`) all four | 4 | mid (no local template; needs judgment on crate layout) |
| 4 | promote real-binary `checked=N,total=N` into §13 for `route`/`store` | 2 | junior |
| 5 | move stray Cargo.toml snippet §11 → §7 in `store`/`user_cli` | 2 | junior |

Gate before the next review: the extraction script in this file's Method block should be committed as
a checker that exits nonzero on any node failing any of the four — a check cheaper to fake than to
satisfy will be faked (PRINCIPLES), and three waves of per-area spot-fixing is exactly that failure.
