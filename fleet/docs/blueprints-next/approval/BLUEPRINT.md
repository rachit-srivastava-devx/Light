# BLUEPRINT — `approval`

## 1. Identity and LLD path
- Node id / label / tag: `approval / Publication Approval / deterministic`
- LLD authority: `docs/LLD/LLD.md §1, §13, §15, §21`, `docs/LLD/LLD-META-L8-ADDENDUM.md §C, §H`, and `docs/LLD/lld-full-detail.architecture.json:components[id=approval]`
- Why this node exists: mint a narrow, expiring capability for an exact publication proposal.
- Incoming edges: `post -> approval` (`passing post evidence`); operator/user decision.
- Outgoing edges: `approval -> broker` (`PR draft/publication grant`); `approval -> notify` (`state change`); `approval -> user_cli` explanation.
- Build status: `partial` — `crates/fleet-lifecycle/src/human_approval.rs:10-25` and `edges_attested.rs:10-22` provide a human-gated transition seam, not persisted scoped grants.

## 2. Responsibility and non-goals
**Owns:** approval record, scope/action/content binding, expiry/revocation/replay checks, and decision receipt.

**Does not own:** judging quality, creating the PR, invoking providers, notification delivery, or rollback.

## 3. Boundary and authority
Only the parent/operator boundary may mint or revoke approval. A worker result, model statement, or existing lifecycle state cannot imply approval. The approval is a capability for one action/resource/content digest and may be consumed once. Persist pending/approved/revoked records before returning success; rejection also writes a receipt.

## 4. Crate/package layout
```text
crates/approval/
  Cargo.toml
  src/lib.rs                 # exports, ≤40 lines
  src/grant.rs               # scoped capability, ≤80 lines
  src/check.rs               # exact-match/expiry/replay, ≤80 lines
  src/port.rs                # persistence/clock ports, ≤60 lines
  tests/approval.rs          # restart/replay cases, ≤80 lines
```

## 5. Public API contract
```rust
pub struct PublicationRequest { pub run_id: String, pub changeset_id: Option<String>, pub destinations: Vec<String>, pub content_digest: String, pub acceptance_digest: String }
pub struct ApprovalRequest { pub task_id: String, pub action: String, pub resource: String, pub scope_hash: String, pub base_commit: String, pub artifact_id: String, pub changeset_id: Option<String>, pub ordered_repo_ids: Vec<String>, pub expires_at: u64 }
pub struct ApprovalGrant { pub approval_id: String, pub request: ApprovalRequest, pub actor: String, pub issued_at: u64 }
pub trait ApprovalStore { fn put(&mut self, grant: &ApprovalGrant) -> Result<(), ApprovalError>; fn consume(&mut self, id: &str) -> Result<ApprovalGrant, ApprovalError>; }
pub fn approve(store: &mut impl ApprovalStore, req: ApprovalRequest, actor: String, now: u64) -> Result<ApprovalGrant, ApprovalError>;
```
Preconditions: action/resource/scope/base/artifact nonempty, expiry is future, actor is operator-authorized. Postcondition: one exact grant, never a boolean; no panic.

## 6. Data model, invariants, and failure taxonomy
| Type / record | Invariant | Illegal state prevented | Failure / exit code |
|---|---|---|---|
| grant | exact task/action/resource/scope/base/artifact and expiry | confused-deputy publication | `ScopeMismatch/7` |
| approval record | unique ID, actor, timestamps, decision, receipt sequence | untraceable decision | `Receipt/8` |
| consumed grant | one-use and not expired/revoked | replay | `Replay/6` |
| persistence | record durable before success | restart loses approval | `Store/3` |

Clock, ID source, authorization, and store are injected; timestamps/counts are integers.

## 7. Reuse map and adoption decisions
| Existing source or stable library | Evidence/version/commit/license | Use | Why not custom code | Proof still required |
|---|---|---|---|---|
| `crates/fleet-lifecycle/src/human_approval.rs:10-25` | exact local source | evidence-bearing human gate seam | preserve existing lifecycle API | scoped persistent grant |
| `crates/fleet-lifecycle/src/edges_attested.rs:10-22` | local source seam | accept transition integration | no second lifecycle transition | replay/refusal contract |
| `crates/fleet-lifecycle/src/receipt.rs:11-22` | local trait | receipt port | common audit authority | durable store smoke |
| SQLite authority (`fleet-store`) | current local binding/version must be recorded at implementation | restart-safe records | no in-memory approval singleton | file-backed restart test |

No UI/provider is adopted. Approval does not prove provider publication capability.

### Cargo.toml snippet (pinned)
```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
thiserror = "2"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

## 8. Behavior matrix
### `approve` / `consume`
| Input/failure dimension | Observable behavior |
|---|---|
| empty / missing | refusal receipt; no grant |
| wrong type / Unicode | typed decode refusal; actor/reason remains data |
| huge / negative | bounded reason/resource; negative or overflow timestamp refuses |
| duplicate / concurrent | duplicate scope request returns conflict; one consume wins, second is replay refusal |
| partial failure / timeout | persistence failure means no success; provider is not contacted |
| stale / unavailable | expired/revoked/base-mismatch grant refuses; unavailable store is environment fault 3 |

## 9. Tiny implementation steps
1. In `src/lib.rs` and `src/grant.rs`: define `ApprovalRequest`, `ApprovalGrant`, and `ApprovalError` types → `cargo check -p approval` exits 0.
2. In `src/check.rs`: port HumanApproval evidence without treating it as the grant → add `approval::tests::grant_consumed_on_approval` and run `cargo test -p approval grant_consumed_on_approval` exits 0.
3. In `src/check.rs`: implement exact scope/expiry/replay predicate → add `approval::tests::replay_refused_after_consume` and run `cargo test -p approval replay_refused_after_consume` exits 0.
4. In `src/port.rs`: add `ApprovalStore` durable implementation and receipt transaction as one atomic operation → add `approval::tests::restart_survives_consume` and run `cargo test -p approval restart_survives_consume` exits 0.
5. In `tests/approval.rs`: run real-binary smoke — approve, restart, consume once, replay → `target/debug/fleet approval --fixture tests/fixtures/blueprint-approval/exact-digest.json --restart` output contains `checked=2,total=2`.

## 10. Test matrix
**Unit tests:** expiry, scope mismatch, base mismatch, revoke, duplicate, one-use consume.

**Integration/contract tests:** passing post verdict creates a broker input only after grant consumption; notify receives state change.

**Hidden tests:** worker-supplied approval field, clock rollback, restart before consume, artifact changed after approval, two consumers racing.

**Property tests:** no grant with any mismatched field can be consumed; 500 generated grants.

**Differential tests:** HumanApproval lifecycle acceptance versus approval adapter; no divergence on evidence-required transitions.

**Real-binary/effect test:** `target/debug/fleet approval --fixture ...`; restart state dir and assert one consume plus one replay receipt.

### Named integration tests (these 3 must compile and pass)

| Test name (exact) | Inputs | Expected output | Mutation / anti-stub caught |
|---|---|---|---|
| `approval::tests::grant_consumed_on_approval` | `ApprovalRequest` with exact scope/action/resource/base/artifact, future `expires_at`, mock `ApprovalStore` | `ApprovalGrant` returned and persisted in the store; store contains exactly one record | catches any stub that returns a grant without persisting it; also catches wrong-scope bindings |
| `approval::tests::replay_refused_after_consume` | same `ApprovalGrant` id consumed twice against the same `ApprovalStore` | first consume returns `Ok(ApprovalGrant)`; second returns `Err(ApprovalError::Replay)` | catches consume-once guard removal; a read-only consume lets the second call succeed |
| `approval::tests::expired_grant_refused` | `ApprovalGrant` with `expires_at` set to a past timestamp (clock injected) | `Err(ApprovalError::Expired)` before any store write | catches expiry-check removal; a hard-coded `Ok` also fails |

## 11. Mutation targets and anti-stub proof

| Function mutated | Mutation applied | Test that catches it | Why it's the right catcher |
|---|---|---|---|
| `validate_grant` in `src/check.rs` | Remove scope-hash equality check — accept any scope | `approval::tests::grant_consumed_on_approval` | test binds an exact scope; a mismatched scope still gets a grant from the mutant, but the binding assertion on the returned grant's `scope_hash` fails |
| `consume` in `src/check.rs` | Change consume to a read-only fetch — do not mark the grant consumed | `approval::tests::replay_refused_after_consume` | test calls consume twice with the same id; a read-only consume lets the second call return `Ok` and the `Err(Replay)` assertion fails |
| `validate_grant` in `src/check.rs` | Skip expiry check — return `Ok` for any timestamp | `approval::tests::expired_grant_refused` | test uses a past `expires_at`; removing the check makes the function return `Ok` instead of `Err(Expired)` |
| `approve` in `src/check.rs` | Infer approval from existing lifecycle state rather than requiring explicit actor decision | `approval::tests::grant_consumed_on_approval` | test provides no lifecycle state; the mutant derives a grant from state alone, breaking the actor-binding assertion |
| `consume` in `src/check.rs` | Return success without durably writing the consumption record | `approval::tests::replay_refused_after_consume` | test restarts the store and replays; a non-durable consume lets the replay succeed after restart |

Safety mutation floor: `caught/total >= 80%`. Reviewer manually enables the read-only consume mutation and confirms `replay_refused_after_consume` goes red.

## 12. Verification recipe and denominators
```bash
cargo test -p approval --no-fail-fast
cargo clippy -p approval --all-targets -- -D warnings
find crates/approval -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +
target/debug/fleet approval --fixture tests/fixtures/blueprint-approval/exact-digest.json --restart
# Mutation floor: caught/total >= 80% (authority)
```
Expected: unit decisions `checked=10,total=10`; property grants `checked=500,total=500`; restart smoke `checked=2,total=2` (consume/replay); no provider call is claimed.

## 13. Definition of done
All of the following must be true — each is checkable by command output, no subjective criteria:

- `cargo test -p approval --no-fail-fast` exits 0 with `test result: ok` in output.
- `approval::tests::grant_consumed_on_approval` appears in test output and passes.
- `approval::tests::replay_refused_after_consume` appears in test output and passes.
- `cargo clippy -p approval --all-targets -- -D warnings` exits 0 (zero warnings).
- `find crates/approval -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +` exits 0.
- `target/debug/fleet approval --fixture tests/fixtures/blueprint-approval/exact-digest.json --restart` output contains `checked=2,total=2`.
- Mutation floor: `caught/total >= 80%` for the 3 named mutation targets in §11 (scope hash, expiry, replay).
- `Cargo.toml` pins `serde`, `thiserror`, and `tokio` exactly as shown in §7.

## 14. Failure stories and review questions
**Failure → Cause → Fix:** an approval survived a changed artifact → approval was a task boolean → bind action, resource, base, and content digest and reject stale scope.

1. Who can mint? 2. What exact bytes are approved? 3. What does restart/replay do?
