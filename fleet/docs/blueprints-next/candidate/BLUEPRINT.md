# BLUEPRINT — `candidate`

## 1. Identity and LLD path
- Node id / label / tag: `candidate / Learning Candidate / deterministic`
- LLD authority: `docs/LLD/LLD.md §11, §19` and `docs/LLD/lld-full-detail.architecture.json:components[id=candidate]`
- Why this node exists: convert verified failure evidence into a scoped, non-active lesson candidate.
- Incoming edges: `verify -> candidate` (`Evidence` containing task type, failure signature, tree/evidence digests, and `{checked,total}`).
- Outgoing edges: `candidate -> offline` (`CandidateLesson` plus regression fixture); no direct policy write.
- Build status: `partial` — `crates/fleet-memory/src/promote.rs:10-72`, `gate_check.rs:1-35`, and `crates/fleet-plan/src/teach.rs` are reusable seams, not the complete LLD node.

## 2. Responsibility and non-goals
**Owns:** canonical failure signature, scope/applicability predicate, candidate status, provenance, counterexample slots, and regression-fixture reference.

**Does not own:** deciding whether a failure is real (`verify`), running paired trials (`offline`), activating standards (`knowledge`), or external publication (`broker`).

## 3. Boundary and authority
Deterministic controller-owned transform. Inputs are untrusted evidence but must be digest-bound to the exact verified tree. The node writes only a candidate record and receipt through a store port; it cannot grant capability, alter policy, or infer success from a model narrative. Empty evidence, missing digests, or `checked=0` refuse with a receipt.

## 4. Crate/package layout
```text
crates/candidate/
  Cargo.toml                 # typed candidate dependencies
  src/lib.rs                  # exports, ≤40 lines
  src/types.rs                # records/status/errors, ≤80 lines
  src/build.rs                # deterministic candidate construction, ≤80 lines
  src/port.rs                 # store/receipt ports, ≤60 lines
  tests/build.rs              # candidate and refusal cases, ≤80 lines
```

## 5. Public API contract
```rust
pub struct Evidence { pub task_type: String, pub signature: String, pub tree_digest: String, pub evidence_digest: String, pub checked: u64, pub total: u64 }
pub struct CandidateLesson { pub id: String, pub scope: String, pub fixture_digest: String, pub provenance: Vec<String>, pub status: Status }
pub trait CandidateStore { fn insert(&mut self, c: &CandidateLesson) -> Result<(), CandidateError>; }
pub fn build(e: Evidence, fixture_digest: String) -> Result<CandidateLesson, CandidateError>;
```
Preconditions: nonempty IDs/digests, `checked > 0`, `checked == total`, and a fixture digest. Postcondition: status is `Candidate`, never `Validated`; no panic and no float threshold.

## 6. Data model, invariants, and failure taxonomy
| Type / record | Invariant | Illegal state prevented | Failure / exit code |
|---|---|---|---|
| `Evidence` | exact tree/evidence digests and `0<checked==total` | lesson from skipped gate | `Coverage/6` |
| `CandidateLesson` | scoped predicate, fixture, provenance, status=`Candidate` | global self-training | `Invalid/7` |
| candidate receipt | unique candidate ID and source sequence | duplicate insertion | `Conflict/8` |
| store write | insert and receipt are one durable operation | orphaned lesson | `Store/3` |

State is single-writer through the parent store; timestamps and IDs are injected. Serialization is versioned canonical JSON with fixed strings for hashes and integer counts.

## 7. Reuse map and adoption decisions
| Existing source or stable library | Evidence/version/commit/license | Use | Why not custom code | Proof still required |
|---|---|---|---|---|
| `crates/fleet-memory/src/promote.rs:10-72` | graph/source exact lines; local | scope/status/refusal vocabulary | preserve existing lesson boundary | candidate fixture parity |
| `crates/fleet-memory/src/gate_check.rs:4-35` | graph/source exact lines; local | added-line pattern check | reuse memory gate semantics | nonempty gate integration |
| `crates/fleet-plan/src/teach.rs` | source seam; local | teaching/provenance fixture discovery | avoid a second lesson format | exact schema contract |
| `serde`/`serde_json` workspace | local lockfile; versions must be read at implementation time | versioned receipt serialization | maintained codec | `cargo tree` plus round-trip smoke |

No external provider is required. Current `fleet-memory` uses `f64` for relevance/importance; that is not adopted for candidate acceptance thresholds, which use integer counts and fixed-point strings if a score is later needed.

### Cargo.toml snippet (pinned)
```toml
[dependencies]
serde      = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror  = "2"
tokio      = { version = "1", features = ["rt-multi-thread", "macros"] }
```

## 8. Behavior matrix
### `build`
| Input/failure dimension | Observable behavior |
|---|---|
| empty / missing | typed refusal and refusal receipt; no store write |
| wrong type / Unicode | decode refusal; Unicode remains opaque in the signature |
| huge / negative | bounded signature/provenance sizes; negative counts refuse |
| duplicate / concurrent | same candidate digest is idempotent; conflicting fixture is `Conflict` |
| partial failure / timeout | no candidate success; retry only with same idempotency key |
| stale / unavailable | stale tree or unavailable store refuses; never widens scope |

## 9. Tiny implementation steps
1. In `src/lib.rs` and `src/types.rs`: define `Evidence`, `CandidateLesson`, `Status`, and `CandidateError` types → `cargo check -p candidate` exits 0.
2. In `src/build.rs`: implement digest/coverage predicate — reject `checked=0`, `checked!=total`, and digest mismatch → add `candidate::tests::zero_coverage_refused` and run `cargo test -p candidate zero_coverage_refused` exits 0.
3. In `src/build.rs`: port `promote.rs` scope/provenance vocabulary for deterministic candidate construction → add `candidate::tests::verify_candidate_offline_contract` and run `cargo test -p candidate verify_candidate_offline_contract` exits 0.
4. In `src/port.rs`: add atomic `CandidateStore` insert and receipt as one durable operation → add `candidate::tests::duplicate_candidate_conflicts` and run `cargo test -p candidate duplicate_candidate_conflicts` exits 0.
5. In `tests/build.rs`: exercise the real verify-to-candidate composition path → `target/debug/fleet pipeline_probe --candidate-fixture tests/fixtures/blueprint-candidate/failed-gate.json` output contains `checked=1,total=1`; one candidate receipt persisted.

## 10. Test matrix
**Unit tests:** valid candidate, empty signature, mismatched digest, `checked=0`, duplicate fixture.

**Integration/contract tests:** `verify_candidate_offline_contract` — exact evidence digest reaches offline input unchanged.

**Hidden tests:** model-provided scope widening, omitted counterexample field, duplicate ID with different fixture, and store failure after receipt preparation.

**Property tests:** generated evidence either yields a candidate only when `0<checked==total` and all digests are nonempty; fixed seed, 1,000 cases.

**Differential tests:** overlap with `fleet-memory::promote_lesson`; divergence allowed only for explicit `Candidate` status and integer denominator enforcement.

**Real-binary/effect test:** `target/debug/fleet pipeline_probe` with a verified fixture; assert candidate receipt is present; live central learning is not required and is not claimed.

### Named integration tests (these 3 must compile and pass)

| Test name (exact) | Inputs | Expected output | Mutation / anti-stub caught |
|---|---|---|---|
| `candidate::tests::verify_candidate_offline_contract` | `Evidence` with nonempty `task_type`, `signature`, matching `tree_digest`/`evidence_digest`, `checked=total=3`, and a fixture digest | `CandidateLesson` with `status=Candidate`, nonempty `fixture_digest`, and `provenance` containing the source `tree_digest` | catches provenance-drop — any implementation that strips the tree digest from provenance fails the assertion |
| `candidate::tests::zero_coverage_refused` | `Evidence` with `checked=0, total=0` | `Err(CandidateError::Coverage)` and no store write | catches the guard preventing zero-input lessons; a stub returning `Ok` for any input fails |
| `candidate::tests::duplicate_candidate_conflicts` | same `Evidence` with the same idempotency key inserted twice | first call returns `Ok(CandidateLesson)`; second returns `Err(CandidateError::Conflict)` | catches missing duplicate detection; a store inserting on every call lets both succeed and the second assertion fails |

## 11. Mutation targets and anti-stub proof

| Function mutated | Mutation applied | Test that catches it | Why it's the right catcher |
|---|---|---|---|
| `build` in `src/build.rs` | Remove `checked == 0` guard — return a candidate for zero-coverage evidence | `candidate::tests::zero_coverage_refused` | test sends `checked=0`; removing the guard makes `build` return `Ok` instead of `Err(Coverage)` |
| `build` in `src/build.rs` | Drop `tree_digest` from the candidate's `provenance` field | `candidate::tests::verify_candidate_offline_contract` | test asserts the exact tree digest appears in `provenance`; stripping it breaks the provenance assertion |
| `insert` in `src/port.rs` | Skip duplicate detection — always insert regardless of existing key | `candidate::tests::duplicate_candidate_conflicts` | test inserts the same evidence twice; skipping the conflict check lets the second insert succeed and the `Err(Conflict)` assertion fails |
| `build` in `src/build.rs` | Widen scope to `*` — ignore the scoped predicate | scope hidden test | candidate cannot grant universal applicability; scope-widen is caught by the hidden test |
| `build` in `src/build.rs` | Skip fixture digest requirement — build a candidate with an empty fixture | offline contract test | every candidate must have a regression anchor; an empty fixture fails the fixture-nonempty assertion |

Reviewer manually removes the coverage predicate; the zero-input test must fail.

Safety mutation floor: `caught/total >= 75%`. Reviewer manually enables one bypass mutation and confirms kill.

## 12. Verification recipe and denominators
```bash
cargo test -p candidate --no-fail-fast
cargo clippy -p candidate --all-targets -- -D warnings
find crates/candidate -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +
target/debug/fleet pipeline_probe --candidate-fixture tests/fixtures/blueprint-candidate/failed-gate.json
# Mutation floor: caught/total >= 75%
```
Expected evidence: unit cases `checked=8,total=8`; property cases `checked=1000,total=1000`; composition `checked=1,total=1`; no skipped tests. A zero-input candidate gate fails.

## 13. Definition of done
All of the following must be true — each is checkable by command output, no subjective criteria:

- `cargo test -p candidate --no-fail-fast` exits 0 with `test result: ok` in output.
- `candidate::tests::verify_candidate_offline_contract` appears in test output and passes.
- `candidate::tests::zero_coverage_refused` appears in test output and passes.
- `cargo clippy -p candidate --all-targets -- -D warnings` exits 0 (zero warnings).
- `find crates/candidate -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +` exits 0.
- `target/debug/fleet pipeline_probe --candidate-fixture tests/fixtures/blueprint-candidate/failed-gate.json` output contains `checked=1,total=1`.
- Mutation floor: `caught/total >= 75%` for the 3 named mutation targets in §11 (zero-input, digest-drop, scope-widen).
- `Cargo.toml` pins `serde` and `thiserror` exactly as shown in §7.

## 14. Failure stories and review questions
**Failure → Cause → Fix:** one transient failure became a global standard → applicability was omitted → require scoped predicate, held-out evaluation, and explicit promotion later.

1. Can a zero-input verifier create a lesson? 2. Can candidate output activate policy? 3. Is the exact tree digest retained?
