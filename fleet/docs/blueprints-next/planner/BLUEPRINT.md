# BLUEPRINT — `planner`

## 1. Identity and LLD path

- Node id / label / tag: `planner / Module Planner / model,ungated`
- LLD authority: `docs/LLD/LLD.md §7, §8` and `docs/LLD/lld-full-detail.architecture.json:components[id=planner]`
- Why this node exists: turn an admitted workflow and compiled evidence into a bounded, reviewable module plan.
- Incoming edges: `dag -> planner` (`recipe`); `context -> planner` (`compiled manifest`).
- Outgoing edges: `planner -> plan_review` (`draft blueprint`).
- Build status: `partial` — `src/dispatch/plan_cmd.rs:117-144` and `crates/fleet-plan` exist, but not this isolated LLD node.

## 2. Responsibility and non-goals

**Owns:** model-proposed module decomposition, dependency declarations, acceptance references, write-set proposal, and plan explanation.

**Does not own:** readiness, grants, scheduling, durable state, source mutation, acceptance execution, or reviewer approval. `dag`, `control/store`, `ready`, `builder`, and `verify` own those concerns.

## 3. Boundary and authority

The planner is an untrusted model adapter. It receives immutable `PlanInput` and may propose only a schema-valid `PlanDraft`; it cannot authorize effects, write a ledger, choose a grant, or mark a module ready. The controller persists the draft and binds its canonical digest before sending it to `plan-review`. Missing output, invalid schema, unknown node, duplicate module id, or an effect outside the declared workflow is a typed refusal with a receipt owned by the parent.

## 4. Crate/package layout

```text
crates/planner/
  Cargo.toml                 # package and pinned workspace dependencies
  src/lib.rs                  # ≤40 lines, exports only
  src/types.rs                # ≤80 lines, input/draft/error records
  src/propose.rs              # ≤80 lines, model port and validation
  tests/contract.rs           # ≤80 lines, schema and edge contract
```

## 5. Public API contract

```rust
pub trait PlannerModel: Send + Sync {
    fn propose(&self, input: &PlanInput) -> Result<PlanDraft, PlannerError>;
}
pub fn validate_draft(input: &PlanInput, draft: &PlanDraft) -> Result<(), PlannerError>;
pub struct PlanInput { pub task_digest: String, pub recipe_digest: String,
    pub context_digest: String, pub acceptance_refs: Vec<String>, pub max_modules: u32 }
pub struct PlanDraft { pub version: u32, pub modules: Vec<ModuleDraft>, pub explanation: String }
```

Precondition: all digests are fixed strings and `max_modules > 0`. Postcondition: a successful draft has nonempty modules, unique ids, acyclic declared dependencies, and no effect not present in the input. No method may panic; adapter failure is `PlannerError`.

## 6. Data model, invariants, and failure taxonomy

| Type / record | Invariant | Illegal state prevented | Failure / exit code |
|---|---|---|---|
| `PlanInput` | all digests nonempty; max > 0 | planning without authority/context | `InvalidInput` / 7 |
| `ModuleDraft` | id unique; dependencies name existing modules or are external refs | hidden dependency/cycle | `InvalidDraft` / 6 |
| `PlanDraft` | canonical serialization and digest are stable | review of mutable prose | `SchemaOrDigest` / 8 |
| `PlannerError` | every adapter failure typed | provider failure misreported as agent failure | `ProviderUnavailable` / 3, refusal / 7 |

Clock, model, token budget, and serialization are injected. No filesystem/network access is allowed in the planner; the controller is the single writer and owns receipts. Counts are integers; unknown model usage is `None` with a reason.

## 7. Reuse map and adoption decisions

| Existing source or stable library | Evidence/version/commit/license | Use | Why not custom code | Proof still required |
|---|---|---|---|---|
| `crates/fleet-plan/src/review/*` | local source, `serde`/`serde_json` already in workspace; licenses require lockfile audit | reuse contract/verdict-shaped records where compatible | preserve existing typed plan vocabulary | compile and contract test |
| `src/dispatch/plan_cmd.rs:117-144` | exact local seam | map CLI plan request into `PlanInput` | avoid duplicate dispatch semantics | real binary path |
| `serde` + `serde_json`; `schemars` 1.2.2 documented 2026-09-11, MIT | research ledger; `schemars` not locally adopted | generate/validate model wire schema | no handwritten JSON parser | pin/version/license and invalid-fixture smoke |

No custom implementation is introduced where the listed crate already provides the facility.

### Cargo.toml snippet (pinned)
```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
schemars = "1"
```

## 8. Behavior matrix

### `PlannerModel::propose`

| Input/failure dimension | Observable behavior |
|---|---|
| empty / missing | refuse with receipt; no draft |
| wrong type / Unicode | schema refusal; preserve Unicode in diagnostic |
| huge / negative | reject over `max_modules` or negative budget/count; never truncate silently |
| duplicate / concurrent | duplicate module ids refuse; same immutable input may be retried with distinct attempt receipt |
| partial failure / timeout | typed provider/environment fault; no success receipt |
| stale / unavailable | stale context digest refuses; unavailable model is exit 3 |

## 9. Tiny implementation steps

1. In `src/lib.rs`: Re-export `PlanInput`, `PlanDraft`, `PlannerModel`, and `PlannerError`; `cargo check -p planner` exits 0.
2. In `src/types.rs`: Define `PlanInput`, `ModuleDraft`, `PlanDraft`, and typed error records; implement `validate_draft` with uniqueness and cycle checks; `cargo test -p planner rejects_duplicate_ids` passes.
3. In `src/propose.rs`: Implement the `PlannerModel` adapter port and wire `validate_draft` after model output is received; `cargo test -p planner rejects_unknown_dependency` passes.
4. In `src/propose.rs`: Add canonical digest generation and refusal receipt emission so a missing receipt is impossible at the controller boundary; `cargo test -p planner rejects_empty_acceptance` passes.
5. In `tests/contract.rs`: Wire `dag_to_planner_to_plan_review_contract` end-to-end with a real serialized payload; assert draft digest and `checked=module_count,total=module_count`; `cargo test -p planner dag_to_planner_to_plan_review_contract` passes.

## 10. Test matrix

**Unit tests:** `rejects_duplicate_ids`, `rejects_unknown_dependency`, `rejects_empty_acceptance` — assert typed errors and no partial draft.

**Integration/contract tests:** `dag_to_planner_to_plan_review_contract` — real serialized payload and digest, not `is_ok()`.

**Hidden tests:** controller supplies unknown effect, stale context, and oversized module list — planner must refuse.

**Property tests:** generated module graphs preserve uniqueness and cycle rejection; fixed seed, at least 200 cases.

**Differential tests:** canonical draft serialization versus the existing `fleet-plan` serializer; allowed divergence is only field ordering normalized by canonical JSON.

**Real-binary/effect test:** `target/debug/fleet plan ...` with fixture repo; assert stdout, exit code, draft artifact, and receipt. No provider login is claimed.

### Named integration tests (these 3 must compile and pass)

| Test name (exact) | Inputs | Expected output | Mutation / anti-stub caught |
|---|---|---|---|
| `planner::tests::rejects_duplicate_ids` | `PlanDraft` with two `ModuleDraft` entries sharing the same `id` string, passed to `validate_draft` | `Err(PlannerError::InvalidDraft)` naming the duplicate id | catches a `validate_draft` stub that skips uniqueness checks; hidden dependency via duplicated id is the failure mode |
| `planner::tests::rejects_unknown_dependency` | `PlanDraft` where one `ModuleDraft` names a dependency id that does not appear in any other module in the draft | `Err(PlannerError::InvalidDraft)` naming the unknown dependency | catches skipping dependency resolution in `validate_draft`; an unknown declared dep is a silent topology error |
| `planner::tests::dag_to_planner_to_plan_review_contract` | Fully serialized `PlanInput` loaded from `tests/fixtures/blueprint-planner/two-modules.json` fed through a mock `PlannerModel` that returns a two-module draft | `PlanDraft` where `draft.modules.len() == 2`, `checked == 2`, `total == 2`, and `draft` serializes to a stable canonical digest | lld edge contract — confirms the dag→planner→plan_review payload is fully wired; a constant `is_ok()` check or empty draft fails the digest and count assertions |

## 11. Mutation targets and anti-stub proof

| Function mutated | Mutation applied | Test that catches it | Why it's the right catcher |
|---|---|---|---|
| `propose` in `src/propose.rs` | Return a constant `PlanDraft { modules: vec![], explanation: "ok".into(), version: 1 }` regardless of input | `planner::tests::dag_to_planner_to_plan_review_contract` | Nonempty denominator and acceptance refs are enforced; a zero-module draft fails the `checked=2,total=2` assertion |
| `validate_draft` in `src/types.rs` | Remove the cycle check, accepting dependency graphs with cycles | cycle property test | Dependency topology is authoritative; a cycle in the declared deps must produce `InvalidDraft` |
| `validate_draft` in `src/types.rs` | Skip the context digest comparison, accepting any `PlanInput` regardless of `context_digest` | stale digest integration test | Review is bound to exact evidence; a stale context digest must refuse before persistence |
| `propose` in `src/propose.rs` | Return `Ok(draft)` after a provider error without writing a refusal receipt | controller refusal test | Refusal is durable before exit; a missing receipt breaks the audit trail at the controller boundary |

Safety mutation floor: `caught/total >= 80%` for planner safety predicates; reviewer manually enables one bypass mutation and confirms kill.

## 12. Verification recipe and denominators

```bash
cargo test -p planner --no-fail-fast
cargo clippy -p planner --all-targets -- -D warnings
find crates/planner -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +
target/debug/fleet plan --fixture tests/fixtures/blueprint-planner/two-modules.json --json
```

Expected evidence: unit/contract/property `checked>0,total=checked`; draft modules `checked=n,total=n,n>0`; provider/version smoke is documented but unverified until a real adapter runs.

## 13. Definition of done

All of the following must be true — each is checkable by inspection or command output, no subjective criteria:

- `cargo test -p planner --no-fail-fast` exits 0 with `test result: ok` in output.
- `planner::tests::rejects_duplicate_ids` appears in test output and passes.
- `planner::tests::rejects_unknown_dependency` appears in test output and passes.
- `planner::tests::rejects_empty_acceptance` appears in test output and passes.
- `planner::tests::dag_to_planner_to_plan_review_contract` appears in test output and passes.
- `cargo clippy -p planner --all-targets -- -D warnings` exits 0 (zero warnings).
- No source file under `crates/planner/src/` exceeds 80 lines (`find crates/planner/src -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +` exits 0).
- Mutation floor: `caught/total >= 80%` across types and propose targets.

## 14. Failure stories and review questions

**Failure → Cause → Fix:** planner invents a dependency or effect → model prose was treated as authority → validate against recipe, context, and effect ceiling before persistence.

1. Can a constant draft pass with `0/0`? 2. Does stale context invalidate the plan? 3. Is provider capability actually smoke-tested or only declared?
