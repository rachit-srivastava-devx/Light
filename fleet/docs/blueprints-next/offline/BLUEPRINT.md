# BLUEPRINT — `offline`

## 1. Identity and LLD path
- Node id / label / tag: `offline / Offline Evaluator / deterministic`
- LLD authority: `docs/LLD/LLD.md §11, §18, §19` and `docs/LLD/lld-full-detail.architecture.json:components[id=offline]`
- Why this node exists: compare a candidate lesson against baseline and held-out tasks before promotion.
- Incoming edges: `candidate -> offline` (`CandidateLesson`, regression fixture, baseline/variant runners).
- Outgoing edges: `offline -> knowledge` (`OfflineScore` with promote/retain/reject decision).
- Build status: `greenfield` — no complete paired evaluator exists; `crates/fleet-judge/tests/offline/*` is a useful test seam only.

## 2. Responsibility and non-goals
**Owns:** deterministic trial pairing, cohort selection, integer/fixed-point metrics, confidence/coverage checks, and non-effectful recommendation.

**Does not own:** generating candidate lessons, changing policy, invoking external providers without a grant, or activating knowledge.

## 3. Boundary and authority
Controller-owned evaluator. Candidate and task fixtures are immutable inputs. Model/command execution is injected behind a bounded runner; results are observations, not authority. The node writes an evaluation receipt, never a lesson activation. A missing baseline, empty cohort, provider timeout, or unequal pairs is a refusal.

## 4. Crate/package layout
```text
crates/offline/
  Cargo.toml
  src/lib.rs                 # exports, ≤40 lines
  src/types.rs               # trial/score records, ≤80 lines
  src/pair.rs                # deterministic pairing, ≤70 lines
  src/stats.rs               # integer/fixed-point metrics, ≤80 lines
  src/runner.rs              # runner port, ≤70 lines
  tests/evaluator.rs         # fixtures and faults, ≤80 lines
```

## 5. Public API contract
```rust
pub trait TrialRunner { fn run(&self, task: &HeldOutTask, variant: Variant) -> Result<Outcome, OfflineError>; }
pub struct EvaluationRequest { pub candidate_id: String, pub tasks: Vec<HeldOutTask>, pub min_pairs: u64, pub seed: u64 }
pub struct OfflineScore { pub candidate_id: String, pub baseline_pass: u64, pub variant_pass: u64, pub checked: u64, pub total: u64, pub recommendation: Recommendation }
pub fn evaluate(runner: &impl TrialRunner, req: EvaluationRequest) -> Result<OfflineScore, OfflineError>;
```
Precondition: tasks are nonempty, `min_pairs>0`, fixed seed, and every task is run in both arms. Postcondition: `checked==total>0`; recommendation cannot activate policy.

## 6. Data model, invariants, and failure taxonomy
| Type / record | Invariant | Illegal state prevented | Failure / exit code |
|---|---|---|---|
| paired trial | same task/input digest in baseline and variant | incomparable samples | `PairMismatch/8` |
| score | integer counts; no float threshold | fabricated precision | `InvalidMetric/6` |
| cohort | `0<checked==total`, held-out and versioned | zero-input pass | `Coverage/6` |
| runner result | bounded output, model/version metadata when available | unknown cohort | `Unavailable/3` |

Randomness is injected as a recorded integer seed; filesystem/network/model are ports. Evaluation is parallel only within an explicit integer limit and produces deterministic ordering.

## 7. Reuse map and adoption decisions
| Existing source or stable library | Evidence/version/commit/license | Use | Why not custom code | Proof still required |
|---|---|---|---|---|
| `crates/fleet-judge/tests/offline/decisions.rs:8-41` | graph exact test seams | decision fixtures and abstain behavior | preserves known refusal cases | paired evaluator contract |
| `crates/fleet-judge/tests/offline/transport.rs:8-21` | graph exact test seams | transport fault fixtures | reuse typed model failure cases | runner boundary |
| `crates/fleet-judge/src/types.rs:8-28` | local typed criteria/candidate | input vocabulary only | avoid duplicate candidate schema | schema parity |
| Rust integer arithmetic/std | stable primary language library | counts and fixed-point deltas | no statistics dependency needed initially | overflow/property tests |
| `serde 1` | crates.io stable; MIT OR Apache-2.0 | derive serialization for CandidateLesson / ValidatedLesson | no custom encode path | `cargo test -p offline` serialization round-trip |
| `thiserror 2` | crates.io stable; MIT OR Apache-2.0 | typed OfflineError variants | no hand-rolled Display impls | compiles with -D warnings |
| `tokio 1` | crates.io stable; MIT | async runner port and test harness | standard async runtime | integration tests use `#[tokio::test]` |
| `proptest 1` | crates.io stable; MIT OR Apache-2.0 | property-based deterministic pairing tests | 1,000-case fixed-seed coverage without hand-written cases | `proptest!` macro in tests/evaluator.rs |

Exact `Cargo.toml` fragment for `crates/offline/Cargo.toml`:
```toml
[dependencies]
serde       = { version = "1", features = ["derive"] }
thiserror   = "2"
tokio       = { version = "1", features = ["rt-multi-thread", "macros"] }

[dev-dependencies]
proptest    = "1"
```

No provider capability or current model version is claimed. If a statistical crate is later selected, record its primary documentation, exact lock version, license, and real call-site smoke first.

## 8. Behavior matrix
### `evaluate`
| Input/failure dimension | Observable behavior |
|---|---|
| empty / missing | refuse and receipt; `0/0` never passes |
| wrong type / Unicode | typed fixture decode refusal; input text preserved |
| huge / negative | cap task count/output; negative limits or overflow refuse |
| duplicate / concurrent | duplicate task IDs deduplicate only when digests match; same candidate evaluation key is idempotent |
| partial failure / timeout | evaluation is incomplete and cannot recommend promotion; retry preserves seed/pairs |
| stale / unavailable | stale candidate/fixture or unavailable runner yields `Unavailable`, not a score |

## 9. Tiny implementation steps
1. In `src/lib.rs`: Define `EvaluationRequest`, `OfflineScore`, `Recommendation`, `OfflineError`, and `TrialRunner`; re-export all public items; `cargo check -p offline` exits 0.
2. In `src/pair.rs`: Implement deterministic pair construction with digest equality check; `cargo test -p offline lesson_candidate_evaluated_in_paired_trial` exits 0.
3. In `src/stats.rs`: Implement integer pass-count metrics and coverage gate (reject `0/0`); `cargo test -p offline rejected_candidate_not_promoted` exits 0.
4. In `src/runner.rs`: Add injected `TrialRunner` with bounded timeout and persisted trial observations; `cargo test -p offline evaluation_is_deterministic` exits 0.
5. In `tests/evaluator.rs`: Wire all three named integration tests end-to-end; `cargo test -p offline --no-fail-fast` exits 0; fixture run publishes `checked=n,total=n,n>0` and makes no policy write.

## 10. Test matrix
**Unit tests:** pair equality, duplicate conflict, pass counts, overflow, abstain on insufficient pairs.

**Integration/contract tests (named — names must appear verbatim in `tests/evaluator.rs`):**

### Named integration tests (these three must compile and pass)

| Test name | What it sends | What it asserts | Why it is not fakeable |
|---|---|---|---|
| `offline::tests::lesson_candidate_evaluated_in_paired_trial` | `CandidateLesson` with valid task fixtures and `min_pairs>0` | `ValidatedLesson` is produced; `checked==total>0`; receipt contains candidate_id | A stub returning a constant ValidatedLesson still passes — but the next two tests kill it |
| `offline::tests::rejected_candidate_not_promoted` | `CandidateLesson` that fails quality criteria (variant_pass < baseline_pass by configured margin) | `ValidatedLesson` is NOT emitted; function returns `Err` or `None`; no promotion side-effect | A stub that always emits ValidatedLesson fails this test |
| `offline::tests::evaluation_is_deterministic` | Same `CandidateLesson` with identical seed submitted twice | Both calls return byte-identical evaluation results | A non-deterministic stub producing different outputs fails; proves seed is honoured |

**Hidden tests:** candidate sees held-out input, baseline omitted, runner returns same output constant, timeout after one arm, and model metadata absent.

**Property tests:** every generated valid score has `checked==total>0`; mismatched pairs never recommend; 1,000 fixed-seed cases.

**Differential tests:** old offline judge fixtures versus new deterministic evaluator; allowed divergence is explicit integer metric rounding only.

**Real-binary/effect test:** `target/debug/fleet offline --fixture tests/fixtures/blueprint-offline/minimal.json`; assert receipt, nonzero denominator, and unchanged policy file.

## 11. Mutation targets and anti-stub proof

**Named mutation targets:** `evaluate_candidate`, `apply_quality_criteria`, `emit_validated_lesson`.

| Function mutated | Mutation applied | Test that catches it | Why it's the right catcher |
|---|---|---|---|
| `evaluate_candidate` in `src/runner.rs` | Evaluate only the variant arm; skip baseline entirely | `offline::tests::lesson_candidate_evaluated_in_paired_trial` (paired-arm hidden test) | Baseline is required; a one-arm score is not a paired trial — the pairing assertion fails |
| `apply_quality_criteria` in `src/stats.rs` | Accept `0/0` — return `ValidatedLesson` on a zero-task cohort | `offline::tests::rejected_candidate_not_promoted` (coverage test) | Denominator is meaningful; zero-input cannot promote — the absence assertion fails |
| `emit_validated_lesson` in `src/runner.rs` | Return a constant `Recommendation::Promote` regardless of pass/fail counts | `offline::tests::evaluation_is_deterministic` + varied fixture test | Outcome must affect the decision; a constant recommendation kills both tests when input varies |
| `evaluate_candidate` in `src/runner.rs` | Ignore `candidate_id` digest; accept stale input | stale-candidate hidden test | Score must be version-bound to the exact candidate |
| `emit_validated_lesson` in `src/runner.rs` | Treat a runner timeout as a passing outcome | partial-trial hidden test | Incomplete evidence cannot promote; a timeout must produce `Err` or `Recommendation::Reject` |

Reviewer replaces the runner with a constant-success stub; the varied held-out fixture and differential test must fail.

Safety mutation floor: `caught/total >= 75%`; reviewer manually enables one bypass mutation and confirms kill.

## 12. Verification recipe and denominators
```bash
cargo test -p offline --no-fail-fast
cargo clippy -p offline --all-targets -- -D warnings
find crates/offline -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +
target/debug/fleet offline --fixture tests/fixtures/offline.json
# Mutation floor: caught/total >= 75%
```
Expected: unit cases `checked=12,total=12`; property cases `checked=1000,total=1000`; fixture trials `checked=n,total=n` with `n>0`; no live model/provider result is implied.

## 13. Definition of done

All of the following must be true — each assertion is checkable without human judgement:

- `cargo test -p offline --no-fail-fast` exits 0 with `test result: ok`.
- `offline::tests::lesson_candidate_evaluated_in_paired_trial` appears in test output and passes.
- `offline::tests::rejected_candidate_not_promoted` appears in test output and passes.
- `offline::tests::evaluation_is_deterministic` appears in test output and passes.
- `cargo clippy -p offline --all-targets -- -D warnings` exits 0.
- Property test output shows `checked=1000,total=1000` (proptest 1,000 fixed-seed cases).
- `target/debug/fleet offline --fixture tests/fixtures/blueprint-offline/minimal.json` exits 0 and prints `checked=N,total=N` with `N>0`.
- `find crates/offline -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +` exits 0.
- `grep -rn "activate\|policy_write" crates/offline/src/` exits 1 (zero hits — the evaluator never activates policy).
- Mutation floor: `caught/total >= 75%` for the 5 named mutation targets in §11; reviewer manually enables one bypass mutation and confirms kill.

## 14. Failure stories and review questions
**Failure → Cause → Fix:** a lesson “won” on zero baseline tasks → evaluator treated empty as neutral → require paired nonzero cohort and fail closed.

1. Are arms actually paired? 2. What evidence makes a score reproducible? 3. Can a score directly grant activation?
