# BLUEPRINT — `dag`

## 1. Identity and LLD path
- Node id / label / tag: `dag` / Workflow DAG / `deterministic`
- LLD authority: `docs/LLD/LLD.md §§6,8`, `docs/LLD/LLD-META-L8-ADDENDUM.md §H`, and `docs/LLD/lld-full-detail.architecture.json:components[id=dag]`
- Why: version workflow recipes, detect cycles, and emit dependency-ready work without speculative completion.
- Incoming edges: `scan -> dag`: clear/resolved ambiguity and typed intent; `rollback -> dag`: repair/requeue; `knowledge/context` provide recipe inputs.
- Outgoing edges: `dag -> planner`: immutable recipe; `dag -> ready`: low-risk skip path.
- Build status: `partial` — `ModuleGraph::topological_sort` and `src/pipeline/graph.rs` exist, but no proposed versioned scheduler/lease contract.

## 2. Responsibility and non-goals
**Owns:** graph version, adjacency/indegree, cycle detection, ready ordering, invalidation set.
**Does not own:** persistence authority (`store`), model planning, worker launch, grants, merge, or readiness verdict.

## 3. Boundary and authority
Deterministic pure core with a store port for durable versions. It may emit a recipe and ready candidates; controller validates grants/resources before effects. No worker or model can mutate a graph version in place.

## 4. Crate/package layout
`Cargo.toml` declares the node (≤25 lines); `src/lib.rs` exports it (≤30); `src/graph.rs` owns validation/cycles (≤80); `src/ready.rs` owns ready ordering (≤70); `src/port.rs` owns CAS persistence (≤60); `tests/graph.rs` owns fixtures (≤80). All live under `crates/dag/`.

## 5. Public API contract
```rust
pub struct Node { pub id: String, pub depends_on: Vec<String>, pub read_set: Vec<String>, pub write_set: Vec<String> }
pub struct GraphVersion { pub id: String, pub revision: u64, pub nodes: Vec<Node> }
pub struct WorkflowRecipe { pub id: String, pub version: u64, pub nodes: Vec<String>, pub edges: Vec<String>, pub digest: String }
pub struct SkipPlanningSignal { pub run_id: String, pub plan_digest: String, pub reason: String, pub checked: u64, pub total: u64 }
pub enum DagError { Empty, Duplicate(String), MissingDependency(String), Cycle, StaleRevision }
pub struct ReadySet { pub revision: u64, pub ids: Vec<String>, pub checked: u64, pub total: u64 }
pub fn validate(version: &GraphVersion) -> Result<(), DagError>;
pub fn ready(version: &GraphVersion, accepted: &[String]) -> Result<ReadySet, DagError>;
```
Precondition: nonempty graph for a pass; postcondition `checked==total>0`, no speculative dependency is accepted. `GraphVersion` is the validated internal graph. `WorkflowRecipe` and `SkipPlanningSignal` are canonical outbound adapters; both carry their digest/denominator rather than exposing a raw graph or bare skip boolean.

## 6. Data model, invariants, and failure taxonomy
| Type / record | Invariant | Illegal state prevented | Failure / exit code |
|---|---|---|---|
| graph | unique IDs; all deps exist; acyclic | deadlock/cycle | 6 |
| ready set | indegree zero only after accepted deps; stable ID tie-break | premature work | 6 |
| version | immutable revision/digest | stale plan mutation | 6/8 |

Use integer scores and checked counts. State is single-writer through store; pure graph functions are `Send + Sync`.

## 7. Reuse map and adoption decisions
| Existing source or stable library | Evidence/version/commit/license | Use | Why not custom code | Proof still required |
|---|---|---|---|---|
| `crates/fleet-types/src/module.rs:157-201` `ModuleGraph::topological_sort` | graph/source exact lines | extraction reference for indegrees/cycle error | existing deterministic behavior | rewrite to O(V+E) queue/heap and versioned API |
| `crates/fleet-types/src/module.rs:277-321` tests | graph search exact tests | preserve cycle/topology fixtures | lead-owned acceptance evidence | run unchanged |
| SQLite WAL [docs](https://www.sqlite.org/wal.html) / current `fleet-store` | LLD §§5,11 and local package | persist immutable versions | one authority writer | `cargo test -p fleet-store`; migration/transaction smoke |

Current `topological_sort` uses `Vec::remove(0)`/resort; do not claim LLD O(V+E) complexity until replaced and measured.

### Cargo.toml snippet (pinned)
```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
```

## 8. Behavior matrix
### `validate` / `ready`
| Input/failure dimension | Observable behavior |
|---|---|
| empty / missing | empty graph refusal 7; no ready pass |
| huge / negative | bounded node/edge counts; unsigned revision rejects negative |
| duplicate / concurrent | duplicate IDs refusal; stale revision conflict, never merge silently |
| partial failure / timeout | store transaction refusal; no lease/ready effect emitted |
| stale / unavailable | typed stale/store fault; preserve prior version |

## 9. Tiny implementation steps
1. In `src/lib.rs`: Add `Node`, `GraphVersion`, `DagError`, and `ReadySet` types; re-export all public items; `cargo check -p dag` exits 0.
2. In `src/graph.rs`: Port adjacency/indegree cycle check from `ModuleGraph`; `cargo test -p dag cycle_detection_refuses_input` exits 0.
3. In `src/ready.rs`: Add accepted-dependency ready heap with stable lexicographic ID tie-break; `cargo test -p dag ready_set_respects_accepted_deps` exits 0.
4. In `src/port.rs`: Add store CAS/version port; `cargo test -p dag zero_node_graph_is_refused` exits 0.
5. In `tests/graph.rs`: Wire all three named integration tests end-to-end using graph fixtures; `cargo test -p dag --no-fail-fast` exits 0 with `320/320` property cases.

## 10. Test matrix
**Unit:** missing dependency, cycle, deterministic order, accepted dependency gating.
**Integration/contract:** `scan -> dag -> planner/ready` and rollback requeue payload; `checked=16,total=16`.
**Hidden:** duplicate edge, stale version, zero-node graph, overlapping write sets.
**Property:** 256 generated DAGs plus 64 seeded cycles; all valid graphs sort and every cycle rejects (`320/320`).
**Differential:** compare acyclic ordering/cycle result with current `ModuleGraph`; allowed difference is stable O(V+E) implementation and version errors.
**Real-binary/effect:** `./target/debug/fleet` dry-run must print recipe/receipt; blocked until command exists for proposed path.

### Named integration tests (these three must compile and pass)

| Test name (exact) | Inputs | Expected output | Mutation / anti-stub caught |
|---|---|---|---|
| `dag::tests::cycle_detection_refuses_input` | `GraphVersion` with nodes A→B→A (a two-node cycle) | `Err(DagError::Cycle)` returned; no `ReadySet` produced | Omitting the cycle check causes `validate` to return `Ok(())`; test fails on the expected `Err` |
| `dag::tests::ready_set_respects_accepted_deps` | `GraphVersion` with nodes A (no deps), B (depends on A); `accepted = []` | `ReadySet.ids == ["A"]`; B is not included because A has not been accepted | Decrementing B's indegree speculatively causes B to appear in the ready set; test fails on `ids` content |
| `dag::tests::zero_node_graph_is_refused` | `GraphVersion { nodes: vec![], revision: 1 }` | `Err(DagError::Empty)` returned; no `ReadySet` produced | A stub returning `Ok(ReadySet { ids: vec![], checked: 0, total: 0 })` passes the wrong exit path; test fails on the expected `Err` |
| `dag::tests::stale_revision_refused` | `GraphVersion` with `revision = 1` is persisted; then `validate` is called with the same graph but with `revision = 1` followed by `revision = 0` (earlier) | First call succeeds; second call returns `Err(DagError::StaleRevision)` (version comparison via store CAS) | Removing the revision check allows stale updates; test fails on the expected `Err` |

## 11. Mutation targets and anti-stub proof
| Function mutated | Mutation applied | Test that catches it | Why it's the right catcher |
|---|---|---|---|
| `validate` in `src/graph.rs` | Remove the cycle-detection branch; always return `Ok(())` | `dag::tests::cycle_detection_refuses_input` | Test expects `Err(DagError::Cycle)`; removing the check makes `validate` succeed on a cyclic graph |
| `ready` in `src/ready.rs` | Decrement indegree for speculative (non-accepted) nodes | `dag::tests::ready_set_respects_accepted_deps` | B must not appear in the ready set until A is accepted; the mutation admits B prematurely |
| `validate` in `src/graph.rs` | Accept a zero-node `GraphVersion` as `Ok(())` | `dag::tests::zero_node_graph_is_refused` | Test expects `Err(DagError::Empty)`; removing the empty-graph check yields a false pass |
| `apply_tie_break` in `src/ready.rs` | Use insertion order instead of stable lexicographic sort | deterministic property (256 cases) | Output order must be input-permutation-invariant; an unstable sort produces diverging orders on the same graph |
| `check_revision` in `src/port.rs` | Accept any revision without comparing to stored version | `dag::tests::stale_revision_refused` | Stale plan mutation is the primary state-safety invariant |

Safety mutation floor: `caught/total >= 80%`; reviewer manually kills the omitted-cycle and speculative-decrement mutants.

## 12. Verification recipe and denominators
```bash
cargo test -p dag --no-fail-fast
cargo clippy -p dag --all-targets -- -D warnings
find crates/dag -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +
cargo test -p fleet-types -p dag --no-fail-fast
```
Expected `320/320` property cases, integration `16/16`, `checked>0,total>0`; no source file >80 lines. O(V+E) and real binary remain unverified until implementation.

## 13. Definition of done

- `cargo test -p dag --no-fail-fast` exits 0 with `test result: ok`.
- `dag::tests::cycle_detection_refuses_input` appears in test output and passes.
- `dag::tests::ready_set_respects_accepted_deps` appears in test output and passes.
- `dag::tests::zero_node_graph_is_refused` appears in test output and passes.
- `cargo clippy -p dag --all-targets -- -D warnings` exits 0.
- `find crates/dag -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +` exits 0.
- Property test output shows `320/320` (256 valid DAGs + 64 seeded cycles all handled correctly).
- Mutation floor: `caught/total >= 80%` for the 5 named mutation targets in §11; reviewer manually kills the omitted-cycle and speculative-decrement mutants.
- Store CAS and rollback-requeue contracts pass in the integration suite.

## 14. Failure stories and review questions
**Failure → Cause → Fix:** a dependent task ran before its predecessor completed → scheduler treated planned work as accepted → decrement indegree only from durable accepted dependency events.
1. What is the graph revision boundary? 2. Can zero nodes pass? 3. Is current `remove(0)` being misreported as O(V+E)?
