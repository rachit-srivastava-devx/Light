# BLUEPRINT — `next_plan`

## 1. Identity and LLD path
- Node id / label / tag: `next_plan / Next-Module Planner / model,ungated`
- LLD authority: `docs/LLD/LLD.md §7, §8` and `docs/LLD/lld-full-detail.architecture.json:components[id=next_plan]`
- Why this node exists: propose module N+1 while immutable module N builds.
- Incoming edges: `ready -> next_plan` (`N+1 planning starts`), immutable plan/context and builder progress.
- Outgoing edges: proposed plan returns to `plan_review -> ready`; no direct effect.
- Build status: `partial` — `src/pipeline/planahead/orchestrator.rs:41-59` provides bounded queue/log behavior, not overlap-safe LLD contracts.

## 2. Responsibility and non-goals
**Owns:** bounded N+1 proposal, overlap/conflict declaration, crash-resume cursor.
**Does not own:** builder writes, readiness, review, queue authority, or contract invalidation; `builder`, `ready`, `control`, `plan-review` own those.

## 3. Boundary and authority
Ungated model proposal. It may draft only if N’s contract is immutable and the proposed write/measurement set is disjoint. Shared-contract change emits `StaleDescendants` and replan request; it cannot authorize N+1.

## 4. Crate/package layout
```text
crates/next-plan/
  Cargo.toml
  src/lib.rs             # ≤40 lines
  src/overlap.rs         # ≤80 lines
  src/queue.rs           # ≤80 lines
  tests/resume.rs        # ≤80 lines
```

## 5. Public API contract
```rust
pub fn propose_next(input: &NextInput, queue: &mut dyn NextQueue) -> Result<NextProposal, NextError>;
pub struct NextInput { pub current_plan_digest: String, pub current_write_set: Vec<String>, pub current_measure_set: Vec<String>, pub candidate: PlanDraft, pub queue_capacity: u32 }
pub struct NextProposal { pub plan: PlanDraft, pub disjoint: bool, pub parent_digest: String, pub cursor: u64 }
```
No proposal is accepted when overlap is unknown. Queue backpressure is observable, bounded, and typed.

## 6. Data model, invariants, and failure taxonomy
| Type / record | Invariant | Illegal state prevented | Failure / exit code |
|---|---|---|---|
| overlap | write and measure sets disjoint from N | concurrent interference | `Overlap` / 6 |
| proposal | parent digest immutable | stale N+1 | `Stale` / 8 |
| queue | capacity ≥1, cursor monotonic | unbounded growth/lost resume | `Backpressure` / 7 |
| resume log | finished unit never reruns | duplicate work | `CorruptLog` / 8 |

## 7. Reuse map and adoption decisions
| Existing source or stable library | Evidence/version/commit/license | Use | Why not custom code | Proof still required |
|---|---|---|---|---|
| `run_plan_ahead` lines 41-59 | exact graph source; tests cover queue blocking/resume | bounded Tokio channel and `UnitLog` seam | preserve crash/backpressure behavior | adapt typed proposal |
| Tokio 1.53.1, MIT; local dependency | research ledger | async queue | no custom scheduler | real lag test |

**Exact `Cargo.toml` `[dependencies]` block for `crates/next-plan/Cargo.toml`:**
```toml
[dependencies]
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
serde = { version = "1", features = ["derive"] }
thiserror = "2"
```

## 8. Behavior matrix
### `propose_next`
| Input/failure dimension | Observable behavior |
|---|---|
| empty / missing | refuse missing current digest/candidate |
| wrong type / Unicode | typed schema refusal |
| huge / negative | cap modules/queue; negative cursor refuses |
| duplicate / concurrent | cursor/idempotency prevents duplicate proposal |
| partial failure / timeout | preserve N; return typed refusal and log |
| stale / unavailable | shared contract or changed digest invalidates descendants |

## 9. Tiny implementation steps
1. In `src/lib.rs`: Define `NextInput`, `NextProposal`, `NextError`, and re-export `NextQueue`; `cargo check -p next-plan` exits 0.
2. In `src/overlap.rs`: Implement write/measure-set intersection and parent-digest immutability check; `cargo test -p next-plan n1_plan_starts_while_n_builds` exits 0.
3. In `src/queue.rs`: Wrap bounded Tokio channel with monotonic cursor and `UnitLog`; `cargo test -p next-plan duplicate_signal_deduplicated` exits 0.
4. In `src/queue.rs`: Add `StaleDescendants` invalidation event on shared-contract change; `cargo test -p next-plan signal_without_prior_plan_is_noop` exits 0.
5. In `tests/resume.rs`: Wire all three named integration tests end-to-end; `cargo test -p next-plan --no-fail-fast` exits 0 with `queued_checked=N,queued_total=N,N>0` in fixture output.

## 10. Test matrix
**Unit:** disjoint, overlap, unknown overlap, cursor monotonicity.

**Integration/contract tests:**

### Named integration tests (these three must compile and pass)

| Test name | Inputs | Expected output | Mutation caught |
|---|---|---|---|
| `next_plan::tests::n1_plan_starts_while_n_builds` | `NextPlanSignal` sent while module N is actively building (mock builder running) | A new plan generation is triggered; the call returns without blocking on N's completion; queue depth increases by 1 | Any impl that blocks on N before starting N+1 fails: this is the LLD edge contract test (`ready → next_plan`) |
| `next_plan::tests::signal_without_prior_plan_is_noop` | `NextPlanSignal` sent when no prior plan exists in the queue | No panic; returns `Ok(())`; queue depth remains 0; no proposal emitted | Any impl that panics or errors on missing prior state fails |
| `next_plan::tests::duplicate_signal_deduplicated` | `NextPlanSignal` sent twice in rapid succession with the same parent digest | Exactly one plan generation is triggered; queue depth is 1 (not 2); second signal is silently dropped | Any impl that enqueues both signals fails the idempotency invariant |

**Hidden:** builder changes shared API, queue full, process restart after N complete.
**Property:** no proposal overlaps N write/measure sets; fixed seed 250 cases.
**Differential:** existing plan-ahead log replay versus new queue for completed units.
**Real-binary:** `target/debug/fleet plan-ahead --fixture`; assert bounded queue and receipts.

## 11. Mutation targets and anti-stub proof
| Function mutated | Mutant | Test that must fail | Proof |
|---|---|---|---|
| `emit_next_plan_signal` | ignore overlap; emit even when write/measure sets intersect | `n1_plan_starts_while_n_builds` conflict variant | no concurrent unsafe plan admitted |
| `deduplicate_signal` | remove deduplication; enqueue every signal unconditionally | `next_plan::tests::duplicate_signal_deduplicated` | idempotent queue; two identical signals produce exactly one proposal |
| `start_plan_generation` | block on N's completion before starting N+1 | `next_plan::tests::n1_plan_starts_while_n_builds` | overlap-safe async start; generation must begin before N exits |
| (queue capacity) | unbounded queue; no backpressure | lag test | backpressure is real and observable |
| (cursor) | reset cursor on restart | restart test | no duplicate finished work replayed |

Mutation floor `caught/total >= 80%`; manually remove overlap check in `emit_next_plan_signal` to verify conflict test fails.

## 12. Verification recipe and denominators
```bash
cargo test -p next-plan --no-fail-fast
cargo clippy -p next-plan --all-targets -- -D warnings
find crates/next-plan -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +
cargo test --test '*plan_ahead*' --no-fail-fast
target/debug/fleet plan-ahead --fixture tests/fixtures/blueprint-next-plan/disjoint.json --json
```
Report `queued_checked=n,queued_total=n,n>0`, completed units, cursor, and invalidations. Provider/model capability remains unverified.

## 13. Definition of done
All of the following must be true — each is checkable by inspection or command output, no subjective criteria:

- `cargo test -p next-plan --no-fail-fast` exits 0 with `test result: ok` in output.
- `next_plan::tests::n1_plan_starts_while_n_builds` appears in test output and passes.
- `next_plan::tests::signal_without_prior_plan_is_noop` appears in test output and passes.
- `next_plan::tests::duplicate_signal_deduplicated` appears in test output and passes.
- `cargo clippy -p next-plan --all-targets -- -D warnings` exits 0 (zero warnings).
- `target/debug/fleet plan-ahead --fixture tests/fixtures/blueprint-next-plan/disjoint.json --json` output contains `queued_checked=N,queued_total=N` with `N>0`.
- No source file under `crates/next-plan/src/` exceeds 80 lines (check with `wc -l`).
- Mutation floor: manually deleting the overlap check in `emit_next_plan_signal` causes at least one test to fail (caught/total ≥ 80%).
- `Cargo.toml` pins `tokio = { version = "1", ... }`, `serde = { version = "1", ... }`, `thiserror = "2"` exactly as shown in §7.

## 14. Failure stories and review questions
**Failure → Cause → Fix:** N+1 edits an interface N measures → overlap was lexical-only → declare measurement set and invalidate on shared contract.
1. What makes N immutable? 2. Does full queue block rather than grow? 3. Does restart skip completed units?
