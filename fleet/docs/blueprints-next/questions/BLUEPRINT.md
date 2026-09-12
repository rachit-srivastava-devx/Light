# BLUEPRINT — `questions`

## 1. Identity and LLD path
- Node id / label / tag: `questions` / Question Merger / `deterministic`
- LLD authority: `docs/LLD/LLD.md §§6,7` and `docs/LLD/lld-full-detail.architecture.json:components[id=questions]`
- Why: merge independent probe candidates into at most three concise, ranked questions with provenance.
- Incoming edges: each `probe_* -> questions`: `Question[]`/faults.
- Outgoing edges: `questions -> user_cli`: ≤3 questions; answer feedback returns to controller/versioned plan; no direct DAG effect.
- Build status: `partial` — current `merge_questions` is deterministic but caps at four.

## 2. Responsibility and non-goals
**Owns:** validation, deduplication, ranking, cap, and deterministic serialization.
**Does not own:** deciding materiality, generating questions, asking the user, or resolving answers.

## 3. Boundary and authority
Pure deterministic merge. Candidate text/evidence is untrusted. Output can pause for user input but cannot authorize effects. Parent stores answer-to-plan provenance and invalidates stale plan revisions.

## 4. Crate/package layout
`Cargo.toml` declares the node (≤25 lines); `src/lib.rs` exports it (≤30); `src/merge.rs` owns validation/rank/cap (≤80); `src/types.rs` owns records (≤60); `tests/merge.rs` owns fixtures (≤80). All live under `crates/questions/`.

## 5. Public API contract
```rust
pub fn merge(candidates: Vec<Question>, max: NonZeroU8) -> Result<QuestionSet, MergeError>;
pub struct QuestionSet { pub revision: u64, pub items: Vec<Question> } // 0..=3
pub enum MergeError { EmptyWhy, EmptyText, InvalidCap, InvalidRevision }
```
The production call fixes `max=3`; any other cap is test/config input and cannot exceed three. Output order is stable by severity, unblock score, probe kind, and text.

## 6. Data model, invariants, and failure taxonomy
| Type / record | Invariant | Illegal state prevented | Failure / exit code |
|---|---|---|---|
| question | text/why nonempty after validation | generic/no-op question | drop candidate, or refusal if all malformed / 7 |
| set | `0 ≤ len ≤ 3`, unique semantic text | over-questioning | invariant 6 |
| merge | permutation-invariant | race/order drift | deterministic test failure / 8 |

No float threshold is used; similarity must be fixed-point/integer or a documented deterministic token-set metric. Current `merge_questions` at `crates/fleet-scan/src/merge.rs:23-46` uses Jaccard and `truncate(4)`, so it is partial evidence only.

## 7. Reuse map and adoption decisions
| Existing source or stable library | Evidence/version/commit/license | Use | Why not custom code | Proof still required |
|---|---|---|---|---|
| `crates/fleet-scan/src/probe.rs:25-37` | local source | reuse candidate `Question` fields | shared edge schema | new cap contract |
| `crates/fleet-scan/src/merge.rs:23-46` | exact source lines | reuse ranking/dedup shape, change cap to 3 | preserves behavior where compatible | differential tests document cap delta |
| `unicode-segmentation = "1"` | crates.io v1.x — MIT OR Apache-2.0 — **mandatory** | correct character-boundary enforcement when capping text | bespoke Unicode handling is a known correctness footgun | `cargo test -p questions` smoke on non-ASCII input |
| `serde = { version = "1", features = ["derive"] }` | crates.io v1.x — MIT OR Apache-2.0 — **mandatory** | `#[derive(Serialize, Deserialize)]` on `Question` and `QuestionSet` | hand-rolled ser/de diverges from contract at the edge | round-trip JSON test in `tests/merge.rs` |
| `thiserror = "2"` | crates.io v2.x — MIT OR Apache-2.0 — **mandatory** | `#[derive(Error)]` on `MergeError` | manual `Display`/`Error` impl is boilerplate noise | `cargo check -p questions` clean compile |

Exact `Cargo.toml` dependency block for `crates/questions/Cargo.toml`:

```toml
[dependencies]
unicode-segmentation = "1"
serde = { version = "1", features = ["derive"] }
thiserror = "2"
```

## 8. Behavior matrix
### `merge`
| Input/failure dimension | Observable behavior |
|---|---|
| empty / missing | empty candidates -> `QuestionSet{items:[]}` with valid revision; missing revision -> 7 |
| huge / negative | bounded candidate count/bytes; negative config rejected |
| duplicate / concurrent | semantic dedup; pure and order-independent |
| partial failure / timeout | faults become evidence for parent, not synthetic questions |
| stale / unavailable | parent rejects answers with stale revision; merge itself has no dependency |

## 9. Tiny implementation steps
1. In `src/types.rs`: Define `Question`, `QuestionSet`, and `MergeError`; the cap constant in `src/merge.rs` MUST be `3` — the value `4` is illegal in production paths and MUST NOT appear in `src/`; `cargo check -p questions` exits 0.
2. In `src/merge.rs`: Port candidate validation/ranking from `crates/fleet-scan/src/merge.rs:23-46`; write `questions::tests::all_probe_contributions_merged` BEFORE touching the cap so the test is green; `cargo test -p questions all_probe_contributions_merged` exits 0.
3. In `src/merge.rs`: Change the cap constant from legacy `4` to exactly `3`; write `questions::tests::output_capped_at_three` BEFORE touching the constant so the test is red first; `cargo test -p questions output_capped_at_three` exits 0.
4. In `tests/merge.rs`: Add `questions::tests::empty_probe_output_tolerated` and the user-cli contract fixture asserting ≤3 output; `cargo test -p questions empty_probe_output_tolerated` exits 0.
5. In `tests/merge.rs`: Run `cargo test -p questions --no-fail-fast` with all three named tests present; exits 0 with `checked=12,total=12`.

## 10. Test matrix
**Unit:** empty/invalid/dedupe/rank/cap and Unicode cases.
**Integration/contract:** four probe outputs merge into user-facing ≤3 payload; `checked=12,total=12`.
**Hidden:** four distinct high-severity candidates, malformed why, near-duplicate Unicode, answer revision mismatch.
**Property:** 256 permutations yield same set and never >3; `256/256`.
**Differential:** legacy merge compared on cases with ≤3 outputs; allowed divergence only cap 4→3 and typed errors.
**Real-binary/effect:** `fleet` clarification fixture once composition exists; currently blocked.

### Named integration tests (must appear verbatim in `tests/merge.rs`)

| Test name | Inputs | Expected output | Mutation it catches |
|---|---|---|---|
| `questions::tests::output_capped_at_three` | `Vec<Vec<Question>>` from all 4 probes (`probe_business`, `probe_tech`, `probe_learn`, `probe_research`), 2 questions per probe (8 total). All questions are valid (non-empty `text` and `why`). | `QuestionSet` with `items.len() == 3`; `assert_eq!(questions.questions.len(), 3)` passes. | Removing the cap constant (or raising it to 4) allows 4+ questions through; this test fails immediately. |
| `questions::tests::all_probe_contributions_merged` | `Vec<Vec<Question>>` from all 4 probes, each probe supplying ≥1 distinct question with a unique `why` identifying the probe source. | Merged `QuestionSet` where `items` include at least one question whose `why` traces to each of the 4 probe sources (de-dup preserves provenance, not just first-probe output). | Mutating `merge_probe_outputs` to concatenate only the first probe's output means 3 of 4 probe sources disappear from the set; test fails on the missing-provenance assertion. |
| `questions::tests::empty_probe_output_tolerated` | One probe (`probe_learn`) returns an empty `Vec<Question>`; other 3 probes each supply ≥1 valid question. | `QuestionSet` with `0 < items.len() <= 3`; no panic, no `Err` propagation from the empty probe. | Deleting the early-return guard on empty probe input causes an index-out-of-bounds or propagation of an empty-vector error, crashing the merge call; test fails with a panic. |

## 11. Mutation targets and anti-stub proof

| Function mutated | Mutation applied | Test that catches it | Why it's the right catcher |
|---|---|---|---|
| `cap_to_three` in `src/merge.rs` | Change the cap constant from `3` to `4`, allowing a 4-item return | `questions::tests::output_capped_at_three` | LLD limit is three; any 4-item output violates the edge contract to `user_cli`; `assert_eq!(items.len(), 3)` fails |
| `merge_probe_outputs` in `src/merge.rs` | Replace multi-probe concatenation with `probes[0].clone()` (first probe only) | `questions::tests::all_probe_contributions_merged` | Suppressing 3 of 4 probe sources breaks the DAG provenance guarantee; missing-provenance assertion fails |
| `handle_empty_probe` in `src/merge.rs` | Delete the early-return/skip guard on empty probe input | `questions::tests::empty_probe_output_tolerated` | Without the guard, an empty probe panics or propagates an error crashing the whole merge |
| `validate_why` in `src/merge.rs` | Remove the `why.is_empty()` rejection branch | unit validation test | Generic/no-op questions with empty `why` must be dropped, not passed through |
| `rank_by_severity` in `src/merge.rs` | Replace stable sort with an unstable or input-order sort | permutation property (256 permutations) | Deterministic output is a user-experience contract; order must not depend on input order |
| `merge` in `src/merge.rs` | Replace body with `Ok(QuestionSet { items: vec![] })` | `questions::tests::all_probe_contributions_merged` | Stub silently discards all questions; integration catches the empty set |

## 12. Verification recipe and denominators
```bash
cargo test -p questions --no-fail-fast
cargo clippy -p questions --all-targets -- -D warnings
find crates/questions -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +
```
Expected unit/contract `12/12`, property `256/256`; Safety mutation floor: `caught/total >= 75%`; no file >80 lines. Current repository behavior is a blocker until cap mismatch is resolved in implementation.

## 13. Definition of done
- `[ ]` `cargo test -p questions --no-fail-fast` exits 0 with `checked=12,total=12`.
- `[ ]` `questions::tests::output_capped_at_three` is present in `tests/merge.rs` and passes; the assertion `assert_eq!(questions.questions.len(), 3)` is the terminal assertion of that test.
- `[ ]` `questions::tests::all_probe_contributions_merged` is present and passes.
- `[ ]` `questions::tests::empty_probe_output_tolerated` is present and passes.
- `[ ]` `cargo clippy -p questions --all-targets -- -D warnings` exits 0.
- `[ ]` No file under `crates/questions/` exceeds 80 lines.
- `[ ]` `crates/questions/Cargo.toml` declares `unicode-segmentation = "1"`, `serde = { version = "1", features = ["derive"] }`, and `thiserror = "2"` with no version wildcards.
- `[ ]` The literal value `4` does not appear as a cap constant anywhere in `crates/questions/src/`.
- `[ ]` Mutation floor ≥80%: killing the `cap_to_three` mutant must be demonstrated by running the named test on the mutated binary.

## 14. Failure stories and review questions
**Failure → Cause → Fix:** four probes asked four questions despite LLD limit → legacy merge cap was four → change the authority to three and retain a differential record.
1. Can a malformed candidate survive? 2. Is output permutation-invariant? 3. What exact evidence proves the cap?
