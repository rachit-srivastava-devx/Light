# BLUEPRINT — `plan_review`

## 1. Identity and LLD path
- Node id / label / tag: `plan_review / Reviewer Gate / model,gated`
- LLD authority: `docs/LLD/LLD.md §7, §18` and `docs/LLD/lld-full-detail.architecture.json:components[id=plan_review]`
- Why this node exists: independently challenge a module plan before deterministic readiness.
- Incoming edges: `planner -> plan_review` (`draft blueprint`), `context -> plan_review` via review manifest.
- Outgoing edges: `plan_review -> ready` (`accepted digest`); feedback is a versioned plan event.
- Build status: `partial` — `crates/fleet-plan/src/review/contract.rs:30-50` and verdict tests exist, not this node.

## 2. Responsibility and non-goals
**Owns:** independent findings, acceptance/reject/revise recommendation, reviewer identity and input/output digests.
**Does not own:** plan generation, grants, readiness decision, source edits, or test execution; `planner`, `control`, `ready`, `builder`, `verify` own those.

## 3. Boundary and authority
Model-gated. Reviewer must be independent and qualified by configured fresh-trial cohort; model output is advisory until the controller validates schema, identity, plan digest, and findings. No self-review, no direct effect, no ledger write. Refusal/timeout blocks readiness and receives a receipt.

## 4. Crate/package layout
```text
crates/plan-review/
  Cargo.toml
  src/lib.rs              # ≤40 lines
  src/contract.rs         # ≤80 lines
  src/verdict.rs          # ≤80 lines
  tests/independence.rs   # ≤80 lines
```

## 5. Public API contract
```rust
pub trait PlanReviewer: Send + Sync { fn review(&self, input: &ReviewInput) -> Result<ReviewVerdict, ReviewError>; }
pub fn validate(input: &ReviewInput, v: &ReviewVerdict) -> Result<(), ReviewError>;
pub struct ReviewInput { pub plan_digest: String, pub reviewer_id: String, pub worker_id: String, pub evidence_digest: String }
pub struct ReviewVerdict { pub decision: Decision, pub findings: Vec<Finding>, pub input_digest: String, pub output_digest: String, pub checked: u64, pub total: u64 }
```
`worker_id != reviewer_id`; `checked>0`; accept requires `checked==total`, zero findings only when the reviewed checklist was nonempty. All errors typed.

## 6. Data model, invariants, and failure taxonomy
| Type / record | Invariant | Illegal state prevented | Failure / exit code |
|---|---|---|---|
| reviewer identity | independent qualified cohort | self/weak reviewer | `NotIndependent` / 6 |
| verdict | exact plan/evidence/input digest | verdict replay on new plan | `DigestMismatch` / 8 |
| findings | severity and location required | prose-only approval | `MalformedFinding` / 6 |
| timeout | receipt before block | silent missing review | `ReviewerUnavailable` / 3/7 |

Reviewer/model/network ports are injected; controller persists immutable receipt. Usage is provider-reported only when present, otherwise unknown.

## 7. Reuse map and adoption decisions
| Existing source or stable library | Evidence/version/commit/license | Use | Why not custom code | Proof still required |
|---|---|---|---|---|
| `validate_review_contract` lines 30-50 | exact graph/source seam | authority/cycle checks | preserve existing tests | new crate contract test |
| `submission_eligible` lines 24-45 | exact graph/source seam | self-review/state/attempt boundaries | avoid duplicated lifecycle rules | integration |
| `verdict_decision` lines 40-67 | exact graph/source seam | accept/reject/revise semantics | existing typed refusal behavior | digest binding |
| Gemini CLI 0.59.0 documented, Apache-2.0; not locally run | quality research | optional independent reviewer adapter | use provider protocol, not bespoke model | `gemini --version` + structured smoke |
| `serde 1` | crates.io stable; MIT OR Apache-2.0 | derive serialization for PlanProposal / ReviewedPlanDigest / PlanWalkthrough | no custom encode path | `cargo test -p plan-review` serialization round-trip |
| `thiserror 2` | crates.io stable; MIT OR Apache-2.0 | typed ReviewError variants | no hand-rolled Display impls | compiles with -D warnings |
| `tokio 1` | crates.io stable; MIT | async reviewer adapter and test harness | standard async runtime | integration tests use `#[tokio::test]` |

> **C9 rule enforced:** Do NOT use `anthropic-sdk` directly. All model calls must go through the injected `PlanReviewer` port trait defined in this crate. A direct SDK import is a build failure. Use a `FakePlanReviewer` in tests; the real adapter is wired by the composition root in `src/`.

Exact `Cargo.toml` fragment for `crates/plan-review/Cargo.toml`:
```toml
[dependencies]
serde        = { version = "1", features = ["derive"] }
thiserror    = "2"
tokio        = { version = "1", features = ["rt-multi-thread", "macros"] }
```

## 8. Behavior matrix
### `review`
| Input/failure dimension | Observable behavior |
|---|---|
| empty / missing | refuse missing plan/checklist/reason; receipt |
| wrong type / Unicode | schema refusal; Unicode locations preserved |
| huge / negative | bound findings and tokens; negative counts refuse |
| duplicate / concurrent | duplicate verdict digest conflicts; same immutable input is idempotent |
| partial failure / timeout | block `ready`, typed environment/refusal, no accept |
| stale / unavailable | digest mismatch rejects; unqualified model waits/human route |

## 9. Tiny implementation steps
1. In `src/lib.rs`: Define `ReviewInput`, `ReviewVerdict`, `Finding`, `Decision`, `ReviewError`, and `PlanReviewer` trait; re-export all public items; `cargo check -p plan-review` exits 0.
2. In `src/contract.rs`: Port `validate_review_contract` independence predicates (worker != reviewer, qualified cohort); `cargo test -p plan-review proposal_reviewed_by_independent_model` exits 0.
3. In `src/verdict.rs`: Add reviewer adapter and schema validation for malformed output and timeout; `cargo test -p plan-review walkthrough_emitted_before_ready` exits 0.
4. In `src/verdict.rs`: Bind `input_digest`/`output_digest`/`plan_digest` and block ready on rejection; `cargo test -p plan-review review_rejection_blocks_ready` exits 0.
5. In `tests/independence.rs`: Wire all three named integration tests end-to-end using `FakePlanReviewer`; `cargo test -p plan-review --no-fail-fast` exits 0 with `checked=N,total=N,N>0` in fixture output.

## 10. Test matrix
**Unit tests:** self-review, wrong reviewer role, empty reason, exact attempt boundary.

**Integration/contract tests (named — names must appear verbatim in `tests/independence.rs`):**

### Named integration tests (these three must compile and pass)

| Test name | What it sends | What it asserts | Why it is not fakeable |
|---|---|---|---|
| `plan_review::tests::proposal_reviewed_by_independent_model` | `PlanProposal` with `worker_id` set | `ReviewedPlanDigest` is produced by a model whose identity differs from the plan-generating worker (`reviewer_id != worker_id`); `checked>0` | A stub that always approves without checking identity fails; proves the injected reviewer port enforces independence |
| `plan_review::tests::walkthrough_emitted_before_ready` | `PlanProposal` that passes review | `PlanWalkthrough` message is emitted (observable in output channel) before the ready signal is raised | A stub that emits ready without a walkthrough fails; ordering is structurally enforced |
| `plan_review::tests::review_rejection_blocks_ready` | `PlanProposal` that fails review criteria | `ReviewedPlanDigest.approved == false`; ready signal is NOT raised; receipt is durable | A stub that emits ready regardless of approval fails; proves gate is real |

**Hidden tests:** model accepts a plan with missing acceptance, changed evidence, or unpinned dependency.
**Property tests:** generated verdicts cannot accept with incomplete coverage; fixed seed, 200 cases.
**Differential tests:** old `fleet-plan` verdict decisions versus new adapter for supported states.
**Real-binary/effect test:** real `fleet plan-review` fixture; provider adapter is unverified until a real CLI runs.

## 11. Mutation targets and anti-stub proof

**Named mutation targets:** `call_independent_model`, `validate_plan_proposal`, `emit_walkthrough`.

| Function mutated | Mutation applied | Test that catches it | Why it's the right catcher |
|---|---|---|---|
| `call_independent_model` in `src/verdict.rs` | Allow `reviewer_id == worker_id` (skip the independence check) | `plan_review::tests::proposal_reviewed_by_independent_model` | Separation of duties; the independence assertion `reviewer_id != worker_id` fails immediately |
| `emit_walkthrough` in `src/verdict.rs` | Skip walkthrough emission; raise ready signal immediately after verdict | `plan_review::tests::walkthrough_emitted_before_ready` | Ordering is a structural contract; the ordering assertion fails when walkthrough is absent before ready |
| `validate_plan_proposal` in `src/contract.rs` | Emit ready even when `approved == false` | `plan_review::tests::review_rejection_blocks_ready` | Gate is real; a stub that ignores the rejection verdict allows ready when it must be blocked |
| `validate_plan_proposal` in `src/contract.rs` | Accept a stale plan digest (skip digest comparison) | replay hidden test | Verdict must be plan-bound; a stale-digest accept violates the immutability contract |
| `call_independent_model` in `src/verdict.rs` | Return `approved = true` when `checked == 0` | coverage unit test | No empty-review approval; zero-denominator pass is the injection vector |
| `emit_walkthrough` in `src/verdict.rs` | Drop the timeout receipt; silently continue | refusal hidden test | Durable gate evidence must precede any ready signal; a missing receipt leaves no audit trail |

Safety mutation floor: `caught/total >= 80%`; reviewer manually enables self-approval and confirms kill.

## 12. Verification recipe and denominators
```bash
cargo test -p plan-review --no-fail-fast
cargo clippy -p plan-review --all-targets -- -D warnings
find crates/plan-review -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +
target/debug/fleet plan-review --fixture tests/fixtures/blueprint-plan-review/accepted.json --json
gemini --version  # capability smoke only; current research says not locally run
```
Report checklist `checked=n,total=n,n>0`, findings, identity, model/provider and digests. No remote/production claim.

## 13. Definition of done

All of the following must be true — each assertion is checkable without human judgement:

- [ ] `cargo test -p plan-review --no-fail-fast` exits 0; output contains `test plan_review::tests::proposal_reviewed_by_independent_model ... ok`
- [ ] `cargo test -p plan-review --no-fail-fast` output contains `test plan_review::tests::walkthrough_emitted_before_ready ... ok`
- [ ] `cargo test -p plan-review --no-fail-fast` output contains `test plan_review::tests::review_rejection_blocks_ready ... ok`
- [ ] `cargo clippy -p plan-review --all-targets -- -D warnings` exits 0
- [ ] `grep -r "anthropic.sdk\|anthropic_sdk\|use anthropic" crates/plan-review/src/` exits 1 (zero hits — no direct SDK import)
- [ ] `target/debug/fleet plan-review --fixture tests/fixtures/blueprint-plan-review/accepted.json --json` exits 0 and emits `checked=N,total=N` with `N>0`
- [ ] No source file under `crates/plan-review/src/` exceeds 80 lines (`wc -l` check)
- [ ] Mutation floor: `caught/total >= 80%` when cargo-mutants or equivalent targets `call_independent_model`, `validate_plan_proposal`, `emit_walkthrough`

## 14. Failure stories and review questions
**Failure → Cause → Fix:** expensive model alias was called “independent” → no cohort/identity evidence → wait or route human review.
1. What proves reviewer independence? 2. Can accept hide missing acceptance? 3. Does timeout block readiness?
