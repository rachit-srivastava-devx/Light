# BLUEPRINT — `ready`

## 1. Identity and LLD path
- Node id / label / tag: `ready / Ready Contract / deterministic`
- LLD authority: `docs/LLD/LLD.md §7, §13`, `docs/LLD/LLD-META-L8-ADDENDUM.md §G`; `docs/LLD/lld-full-detail.architecture.json:components[id=ready]`
- Why this node exists: make the fixed admission decision before any builder lease.
- Incoming edges: `plan_review -> ready` (`accepted digest`), `dag -> ready` low-risk skip, control/store state.
- Outgoing edges: `ready -> builder` (`lease + module`), `ready -> next_plan` (`N+1 planning starts`).
- Build status: `partial` — `crates/fleet-plan/src/ready_gate/*` and `ModuleGraph::ready_for_execution` are seams, not the LLD gate.

## 2. Responsibility and non-goals
**Owns:** deterministic predicate, violation list, checked denominator, and admission receipt.
**Does not own:** plan content, model review, grants issuance, resource reservation implementation, source edits, or verification.

## 3. Boundary and authority
Controller-owned deterministic authority. It evaluates `ready(module)` exactly: nonempty acceptance, resolved blocking questions, reviewer acceptance of exact digest, pinned dependencies, grants covering effects, exclusive write scope, and available budget/resources. No model can override a false result. A zero-input or unknown field is refusal, not pass.

`resources_available` is not a local guess: `control` supplies it only after a profile reservation
against the Addendum §G `ResourceProfile`; `resource_profile` identifies that profile. `ready`
publishes the profile name and positive `ResourceObservation` denominator in its admission receipt;
sampling, reservation, and cleanup remain outside this pure predicate.

## 4. Crate/package layout
```text
crates/ready/
  Cargo.toml
  src/lib.rs             # ≤40 lines
  src/predicate.rs       # ≤80 lines
  src/receipt.rs         # ≤80 lines
  tests/predicate.rs     # ≤80 lines
```

## 5. Public API contract
```rust
pub fn evaluate(input: &ReadyInput) -> ReadyVerdict;
pub struct ReadyInput { pub acceptance: Vec<String>, pub questions_open: u64, pub reviewer_accepts: bool,
    pub deps_pinned: bool, pub grants_cover: bool, pub write_scope_exclusive: bool,
    pub resources_available: bool, pub resource_profile: String, pub checked: u64, pub total: u64 }
pub struct ReadyVerdict { pub status: Status, pub violations: Vec<Violation>, pub checked: u64, pub total: u64 }
```
Precondition: all seven checks are present; `checked == total > 0`. Postcondition: `Ready` iff every predicate is true. Pure, deterministic, no panic.

## 6. Data model, invariants, and failure taxonomy
| Type / record | Invariant | Illegal state prevented | Failure / exit code |
|---|---|---|---|
| `ReadyInput` | no absent authority fields; integer counts | implicit grants/readiness | `Incomplete` / 7 |
| `ReadyVerdict` | every failed predicate named | opaque denial | `NotReady` / 6 |
| denominator | checked > 0 and equals total for pass | empty green gate | `ZeroCoverage` / 6 |
| receipt | exact plan/effect/write digests | replay | `DigestMismatch` / 8 |

## 7. Reuse map and adoption decisions
| Existing source or stable library | Evidence/version/commit/license | Use | Why not custom code | Proof still required |
|---|---|---|---|---|
| `crates/fleet-plan/src/ready_gate/gate_eval.rs:11-13` | graph exact line; existing total vocabulary tests | reuse total evaluation shape | preserve existing semantics | map all LLD predicates |
| `crates/fleet-types/src/module.rs:138-153` | graph exact line | compare existing readiness boundaries | avoid contradictory predicate | contract parity test |
| serde/JSON existing workspace | local manifest | receipt serialization | no custom format | schema fixture |

No custom implementation is introduced where the listed crate already provides the facility.

### Cargo.toml snippet (pinned)
```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

## 8. Behavior matrix
### `evaluate`
| Input/failure dimension | Observable behavior |
|---|---|
| empty / missing | `Refused` with named missing predicate and receipt |
| wrong type / Unicode | typed decode failure; preserve IDs |
| huge / negative | checked integer validation; negative/overflow refusal |
| duplicate / concurrent | same input deterministic; conflicting revision refused |
| partial failure / timeout | unavailable resource/grant is not ready |
| stale / unavailable | stale reviewer/plan or unavailable budget returns not ready |

## 9. Tiny implementation steps
1. In `src/lib.rs`: Re-export `ReadyInput`, `ReadyVerdict`, `Status`, and `Violation`; `cargo check -p ready` exits 0.
2. In `src/predicate.rs`: Implement `evaluate` with all seven fixed boolean predicates in LLD order; `cargo test -p ready one_false_predicate_refuses` passes.
3. In `src/receipt.rs`: Add `checked/total` denominator validation and canonical receipt serialization; `cargo test -p ready zero_denominator_is_rejected` passes.
4. In `src/predicate.rs`: Add revision/digest comparison against stored reviewer acceptance; `cargo test -p ready stale_reviewer_digest_refuses` passes.
5. In `tests/predicate.rs`: Drive `target/debug/fleet ready --fixture tests/fixtures/blueprint-ready/accepted-module.json --json`; assert lease is absent when verdict is `NotReady`.

## 10. Test matrix
**Unit tests:** one failing predicate at a time, all-pass, zero denominator, exact boundary.
**Integration/contract tests:** accepted reviewer digest → ready → builder lease; low-risk skip still evaluates grants/write scope.
**Hidden tests:** absent field, open question, duplicate write scope, changed plan revision.
**Property tests:** any false input cannot produce `Ready`; fixed seed, 300 cases.
**Differential tests:** existing ready-gate output versus new node for overlapping fixtures.
**Real-binary/effect test:** `target/debug/fleet ready --json`; assert no lease on refusal.

### Named integration tests (these 3 must compile and pass)

| Test name (exact) | Inputs | Expected output | Mutation / anti-stub caught |
|---|---|---|---|
| `ready::tests::one_false_predicate_refuses` | `ReadyInput` with one predicate false (e.g. `reviewer_accepts: false`), all others true, `checked=7,total=7` | `ReadyVerdict { status: NotReady }` with one named `Violation` | catches "always return ready" stub — any hard-coded pass fails here |
| `ready::tests::zero_denominator_is_rejected` | `ReadyInput` with `checked=0, total=0`, all boolean predicates true | `Err(ZeroCoverage)` or `ReadyVerdict { status: ZeroCoverage }` | catches accepting `0/0` as green; the zero-input gate must fail |
| `ready::tests::stale_reviewer_digest_refuses` | `ReadyInput` with `reviewer_accepts: true` but a reviewer digest that does not match the stored plan digest | `ReadyVerdict { status: NotReady }` with `Violation(StaleDigest)` | catches skipping the digest comparison; stale approval must not pass |

## 11. Mutation targets and anti-stub proof

| Function mutated | Mutation applied | Test that catches it | Why it's the right catcher |
|---|---|---|---|
| `evaluate` in `src/predicate.rs` | Hard-code return `Ready` regardless of predicate values | `ready::tests::one_false_predicate_refuses` | Authority cannot be stubbed; a false predicate must produce `NotReady` |
| `evaluate` in `src/predicate.rs` | Remove the `grants_cover` check, always treating it as satisfied | `ready::tests::one_false_predicate_refuses` with `grants_cover: false` | Effect permission is explicit; each predicate is evaluated independently |
| `validate_denominator` in `src/receipt.rs` | Accept `checked=0, total=0` and return a passing verdict | `ready::tests::zero_denominator_is_rejected` | Zero-input gate fails; empty green gate is prevented |
| `evaluate` in `src/predicate.rs` | Skip reviewer digest comparison, always treating digest as current | `ready::tests::stale_reviewer_digest_refuses` | Admission is version-bound; stale approval cannot produce `Ready` |

Safety mutation floor: `caught/total >= 75%`; reviewer manually flips grants predicate.

## 12. Verification recipe and denominators
```bash
cargo test -p ready --no-fail-fast
cargo clippy -p ready --all-targets -- -D warnings
find crates/ready -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +
target/debug/fleet ready --fixture tests/fixtures/blueprint-ready/accepted-module.json --json
```
Publish `checked=7,total=7` only when all seven inputs were actually evaluated; zero and partial evaluation are failures.

## 13. Definition of done
All of the following must be true — each is checkable by inspection or command output, no subjective criteria:

- `cargo test -p ready --no-fail-fast` exits 0 with `test result: ok` in output.
- `ready::tests::one_false_predicate_refuses` appears in test output and passes.
- `ready::tests::zero_denominator_is_rejected` appears in test output and passes.
- `ready::tests::stale_reviewer_digest_refuses` appears in test output and passes.
- `cargo clippy -p ready --all-targets -- -D warnings` exits 0 (zero warnings).
- No source file under `crates/ready/src/` exceeds 80 lines (`find crates/ready/src -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +` exits 0).
- Mutation floor: `caught/total >= 80%` across predicate and receipt targets.

## 14. Failure stories and review questions
**Failure → Cause → Fix:** a model completion bypassed grants → readiness was prose → only deterministic input predicate may create a lease.
1. Are all seven checks evaluated? 2. Does low-risk skip remain safe? 3. Can stale approval pass?
