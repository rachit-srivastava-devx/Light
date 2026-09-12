# BLUEPRINT — route

## 1. Identity and LLD path

- Node id / label / tag: `route / Route Admission / deterministic`
- LLD authority: `docs/LLD/LLD.md §6, §9, §17`, `docs/LLD/LLD-META-L8-ADDENDUM.md §E`; `docs/LLD/lld-full-detail.architecture.json:components[id=route]`
- Why this node exists: Deterministically filter and rank eligible adapters/models under capability, policy, independence, availability, and budget constraints.
- Incoming edges: `intent -> route` typed IntentSpec; `model_catalog -> route` capability snapshot.
- Outgoing edges: `route -> scan` admitted request or durable refusal.
- Build status: `partial`
- Exact workflow path: `user_cli/connectors -> ingest -> store/control -> intent -> route -> scan`.

## 2. Responsibility and non-goals

**Owns:** ordered filters, conservative refusal, expected-cost scoring, stable tie-break, reservation request, and explanation.

**Does not own:** model inference, catalog discovery, provider auth, policy authoring, or persistence implementation.

## 3. Boundary and authority

Route is pure over an immutable runtime snapshot. It cannot call a model or silently add a candidate. A selected route is only an admission decision; `control`/`store` must transactionally reserve budget and persist the decision before launch. Empty candidate sets fail closed with a reason and receipt. Unknown quality/cost data is insufficient evidence, never zero.

## 4. Crate/package layout

```text
crates/route/
  Cargo.toml
  src/lib.rs                 # exports, 35 lines
  src/filter.rs              # capability/policy/quota filters, 80 lines
  src/score.rs               # integer cost/quality scoring, 80 lines
  src/explain.rs             # stage evidence, 70 lines
  tests/mutations.rs         # authority predicate tests, 80 lines
```

## 5. Public API contract

```rust
pub fn admit(intent: &IntentSpec, snapshot: &CatalogSnapshot, budget: &BudgetSnapshot, policy: &PolicySnapshot) -> Result<RouteDecision, RouteRefusal>;
pub struct RouteDecision { pub selected: CandidateId, pub stages: Vec<StageEvidence>, pub reservation: ReservationRequest, pub snapshot_digest: String }
```

The output is deterministic for identical inputs, uses checked integer arithmetic, and includes all rejection reasons and `checked,total` for candidate evaluation.

## 6. Data model, invariants, and failure taxonomy

| Type / record | Invariant | Illegal state prevented | Failure / exit code |
|---|---|---|---|
| candidate set | only catalog snapshot candidates | phantom adapter | 6 |
| eligibility | capability, grant, availability, identity, budget all pass | unsafe launch | 7 |
| score | integer scaled units; no unknown treated as zero | cost distortion | 8 |
| decision | snapshot digest and stage evidence present | unexplainable route | 6 |

Expected cost is `base_price + failure_probability * repair_cost` using configured conservative baselines when cohort data is insufficient. A different model identity is required for gated review roles.

### Cost and failover boundary

Route may estimate and rank only. `control`/`store` own `UsageObservation`, reservation, settlement,
late correction, and the integer conservation invariant from Addendum §E. When no eligible route
exists, route emits a durable refusal; `model_catalog` may publish only qualified alternatives.
Control owns the durable wait/restart fence and retry budget, so failover cannot hot-loop or spend
against an uncommitted reservation.

## 7. Reuse map and adoption decisions

| Existing source or stable library | Evidence/version/commit/license | Use | Why not custom code | Proof still required |
|---|---|---|---|---|
| `crates/fleet-router/src/decide.rs:13-69` | local LV six-stage pure decision pipeline | direct extraction baseline | preserve refusal order and deterministic result | dynamic catalog snapshot and budget formula |
| `src/dispatch/route_cmd.rs:10-40` | local LV composition path | real binary fixture | proves current CLI wiring only | replace default all-capable runtime |
| `petgraph 0.8.3` | MIT OR Apache-2.0; upstream links/checksum in research | only if route graph needs DAG mechanics | avoid hand-rolled graph algorithms | exact-pin cycle probe |

Petgraph is not route authority; current router logic remains Fleet-owned.

### Cargo.toml snippet (pinned)
```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
petgraph = "0.8.3"
```

## 8. Behavior matrix

### `admit`

| Input/failure dimension | Observable behavior |
|---|---|
| empty / missing | empty catalog returns refusal with `checked=0,total=0`; missing intent/policy refuses |
| wrong type / Unicode | malformed IDs/policy refuse; Unicode task class is opaque to route |
| huge / negative | candidate list cap 1024; negative budget/price/probability refuses; checked multiplication detects overflow |
| duplicate / concurrent | duplicate candidate IDs are rejected; concurrent budget reservation is store CAS, not route-local success |
| partial failure / timeout | route is pure and does not timeout; unavailable catalog candidate is filtered with reason |
| stale / unavailable | stale snapshot digest refuses; no model fallback or score fabrication |

## 9. Tiny implementation steps

1. In `src/lib.rs`: Define `RuntimeState`, `Decision`, `RouteRefusal`, and `CandidateId` exports; `cargo check -p route` exits 0.
2. In `src/filter.rs`: Implement capability, policy, and quota filters as named pure predicate functions; `cargo test -p route capability_filter_rejects_missing` passes.
3. In `src/score.rs`: Add catalog snapshot binding and integer cost/quality scoring with conservative floor; `cargo test -p route score_boundary_conservative_floor` passes.
4. In `src/explain.rs`: Add stage evidence and `checked/total` denominator fields with JSON serialization; `cargo test -p route explain_json_contract` passes.
5. In `tests/mutations.rs`: Assert authority predicates with a non-default snapshot fixture and real `fleet route --json`; `cargo test -p route refusal_receipt_contains_first_empty_stage` passes.

## 10. Test matrix

**Unit tests:** each filter, conservative profile floor, tie-break, overflow, unknown evidence, empty set.

**Integration/contract tests:** IntentSpec + model catalog → route → scan payload; refusal receipt has first empty stage.

**Hidden tests:** stale snapshot, builder/reviewer same identity, quota exhaustion, zero candidates, negative price, and phantom candidate.

**Property tests:** 1,000 candidate permutations produce same decision after stable sort; fixed seed.

**Differential tests:** current `fleet-router::decide` versus extracted core for existing fixtures; allowed difference is dynamic snapshot input and explicit stage evidence.

**Real-binary/effect test:** `fleet route --json` with controlled runtime snapshot; reservation commit is tested through control/store, not route alone.

### Named integration tests (these 3 must compile and pass)

| Test name (exact) | Inputs | Expected output | Mutation / anti-stub caught |
|---|---|---|---|
| `route::tests::capability_filter_rejects_missing` | `CatalogSnapshot` containing one candidate whose required capability is absent from the snapshot's declared capability set; valid `IntentSpec` and `BudgetSnapshot` | `Err(RouteRefusal)` with a stage evidence entry naming the capability filter stage | catches removal of the capability filter — any candidate without the required capability must never reach scoring |
| `route::tests::score_boundary_conservative_floor` | `CatalogSnapshot` with one candidate having no historical cost data; `BudgetSnapshot` with a finite limit | `RouteDecision` where `stages` contains a score stage with the configured conservative baseline, not zero | catches treating unknown cost as zero — insufficient evidence must not distort economics by defaulting to free |
| `route::tests::refusal_receipt_contains_first_empty_stage` | `CatalogSnapshot` with zero candidates after all filters applied; valid `IntentSpec` | `Err(RouteRefusal)` whose `stages` list is non-empty and the first entry names the stage that emptied the set | lld edge contract — confirms stage evidence is populated on refusal; catches a no-op refusal that drops the stage list |

## 11. Mutation targets and anti-stub proof

| Function mutated | Mutation applied | Test that catches it | Why it's the right catcher |
|---|---|---|---|
| `filter_by_capability` in `src/filter.rs` | Remove the capability predicate, passing all candidates regardless of supported capabilities | `route::tests::capability_filter_rejects_missing` | No unsupported adapter may be admitted; removing the filter lets the missing-capability candidate reach scoring |
| `score_candidate` in `src/score.rs` | Return the first candidate in input order rather than computing integer cost/quality score | `route::tests::score_boundary_conservative_floor` + permutation property test | Stable score/tie-break is real; choosing by arrival order changes the winner under permutation |
| `apply_conservative_baseline` in `src/score.rs` | Substitute `0` for missing historical cost data rather than the configured conservative baseline | `route::tests::score_boundary_conservative_floor` | No fabricated economics; zero cost distorts the admission decision in favor of unknown candidates |
| `check_empty_candidates` in `src/filter.rs` | Return a hard-coded non-empty `RouteDecision` when the candidate set is empty after filtering | `route::tests::refusal_receipt_contains_first_empty_stage` | Empty route cannot be green; the stage evidence on refusal proves filtering ran |

Reviewer manually changes `first_empty` to last refusal; refusal-order test must fail.

Safety mutation floor: `caught/total >= 80%` for all authority predicates; reviewer manually enables one bypass mutation and confirms kill.

## 12. Verification recipe and denominators

```bash
cargo test -p route --no-fail-fast
cargo clippy -p route --all-targets -- -D warnings
find crates/route -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +
cargo test -p fleet-router --no-fail-fast
cargo run --bin fleet -- route --json
# Mutation floor: caught/total >= 80% (authority)
```

Expected evidence: existing decision fixtures `checked=6,total=6`; permutations `checked=1000,total=1000`; binary route reports a nonempty stage set; dynamic live catalog remains unverified until model-catalog proof.

## 13. Definition of done

All of the following must be true — each is checkable by inspection or command output, no subjective criteria:

- `cargo test -p route --no-fail-fast` exits 0 with `test result: ok` in output.
- `route::tests::capability_filter_rejects_missing` appears in test output and passes.
- `route::tests::score_boundary_conservative_floor` appears in test output and passes.
- `route::tests::refusal_receipt_contains_first_empty_stage` appears in test output and passes.
- `cargo clippy -p route --all-targets -- -D warnings` exits 0 (zero warnings).
- No source file under `crates/route/src/` exceeds 80 lines (`find crates/route/src -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +` exits 0).
- `target/debug/fleet route --fixture tests/fixtures/blueprint-route/route-fixture.json` output contains `checked=1,total=1`.
- Mutation floor: `caught/total >= 80%` across filter and score targets.

## 14. Failure stories and review questions

**Failure → Cause → Fix:** route picked an unavailable cheap model → capability was a static table → catalog snapshot and capability filter precede scoring.

1. What evidence makes a candidate eligible? 2. Can unknown cost win? 3. Is reservation atomic outside this pure node?
