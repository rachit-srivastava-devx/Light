# BLUEPRINT — `rollback`

## 1. Identity and LLD path
- Node id / label / tag: `rollback / Guarded Rollback / deterministic`
- LLD authority: `docs/LLD/LLD.md §14, §15`, `docs/LLD/LLD-META-L8-ADDENDUM.md §C, §D, §H`, and `docs/LLD/lld-full-detail.architecture.json:components[id=rollback]`
- Why this node exists: undo a failed local integration safely or create a compensating proposal when history/effects cannot be reset.
- Incoming edges: `post -> rollback` (`failed post verdict`); operator rollback request.
- Outgoing edges: `rollback -> dag` (`requeue / repair`); compensating candidate/approval for shared or published refs.
- Build status: `partial` — `crates/fleet-merge/src/worktree.rs`, `src/dispatch/ledger_cmd.rs:44-53`, and `keel/fleet/src/main.rs:2428-2661` provide rollback seams and tests, but the LLD shared-ref compensation path is incomplete.

## 2. Responsibility and non-goals
**Owns:** rollback eligibility, current-HEAD/foreign-modification guard, private-ref restoration, compensating-revert proposal, receipts, and reconciliation state.

**Does not own:** deciding that post verification failed, deleting arbitrary paths, resetting shared history, undoing external side effects, or silently requeueing work.

## 3. Boundary and authority
Controller-owned destructive local effect. Before merge, only an owned worktree under the repository’s `.worktrees` root may be removed. After a private integration, restoration is allowed only when current HEAD equals Fleet’s integration result and no foreign modification exists. Shared/published refs require a new human-approved `git revert` proposal; `reset --hard` is forbidden. Every refusal writes a receipt; rollback does not erase failed evidence.

## 4. Crate/package layout
```text
crates/rollback/
  Cargo.toml
  src/lib.rs                 # exports, ≤40 lines
  src/guard.rs               # eligibility/CAS predicates, ≤80 lines
  src/action.rs              # private cleanup/compensation port, ≤80 lines
  src/reconcile.rs           # post-action verification, ≤70 lines
  src/receipt.rs             # rollback evidence, ≤60 lines
  tests/rollback.rs          # fresh-repo and refusal cases, ≤80 lines
```

## 5. Public API contract
```rust
pub enum RollbackTarget { OwnedWorktree(PathBuf), PrivateRef { repo: PathBuf, before: String, applied: String }, SharedRef { repo: PathBuf, merge_commit: String } }
pub struct RepairSignal { pub run_id: String, pub failed_node_id: String, pub reason: String, pub source_revision: u64 }
pub struct RollbackRequest { pub artifact_id: String, pub target: RollbackTarget, pub reason: String, pub grant: Grant }
pub struct CompensationRequest { pub changeset_id: String, pub operation_key: String, pub repo_id: String, pub expected_published_ref: String, pub prior_ref: String, pub grant_id: String }
pub struct RollbackReceipt { pub artifact_id: String, pub status: String, pub before: String, pub after: String, pub checked: u64, pub total: u64 }
pub trait GitRollbackPort { fn remove_owned(&self, path: &Path) -> Result<(), RollbackError>; fn revert(&self, repo: &Path, commit: &str) -> Result<String, RollbackError>; fn head(&self, repo: &Path) -> Result<String, RollbackError>; }
pub fn rollback(git: &impl GitRollbackPort, store: &mut impl RollbackStore, req: RollbackRequest) -> Result<RollbackReceipt, RollbackError>;
pub fn compensate(input: &CompensationRequest, broker: &mut impl BrokerPort, store: &mut impl RollbackStore) -> Result<RollbackReceipt, RollbackError>;
```
Preconditions: target is resolved from durable artifact evidence, grant covers the exact action,
and current state passes guard checks. Shared compensation requires Addendum §C's operation key,
remote readback, and explicit grant. Postcondition: one durable receipt; second rollback is
refusal; no panic.

## 6. Data model, invariants, and failure taxonomy
| Type / record | Invariant | Illegal state prevented | Failure / exit code |
|---|---|---|---|
| target | path/ref is owned and exact | arbitrary deletion/reset | `Containment/6` |
| private rollback | current HEAD equals applied and before is known | clobber later work | `CasMismatch/8` |
| shared rollback | only compensating revert proposal, never reset | history rewrite | `NeedsApproval/7` |
| receipt | artifact preserved, one terminal status, checked/total nonzero | lost failure evidence | `Receipt/8` |

The action is parent-only, lock-serialized, and uses collision-safe `mktemp`/TempDir for scratch. External side effects and DB migrations are classified `NeedsReconciliation`, not “undone” by Git.

## 7. Reuse map and adoption decisions
| Existing source or stable library | Evidence/version/commit/license | Use | Why not custom code | Proof still required |
|---|---|---|---|---|
| `src/dispatch/ledger_cmd.rs:44-53` | graph exact source | CLI rollback entry and error port | preserve command contract | guarded node API |
| `keel/fleet/src/main.rs:2428-2514` | graph exact source | refusal receipt, already-rolled-back/unapplied checks | retain known safety cases | fresh-repo guard |
| `keel/fleet/src/main.rs:2605-2661` | graph exact source | isolated apply/reverse verification shape | preserve evidence-before-cleanup | no shared reset |
| Git worktree/merge/revert CLI | primary Git docs; runtime version unverified here | authoritative mutation | `git2` not adopted for mutation | real binary/fresh repo |

The existing `fleet_merge::remove` path is not sufficient by itself: it must be wrapped with path containment, artifact state, current-HEAD CAS, and durable receipt checks.

### Cargo.toml snippet (pinned)
```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
tokio = { version = "1", features = ["rt-multi-thread", "process", "macros"] }
```

## 8. Behavior matrix
### `rollback`
| Input/failure dimension | Observable behavior |
|---|---|
| empty / missing | invalid artifact/target refuses and writes receipt |
| wrong type / Unicode | malformed ID/path refuses; path is never interpreted as shell text |
| huge / negative | bounded artifact/diff/log sizes; invalid counts/timeouts refuse |
| duplicate / concurrent | lock serializes; prior `rolled_back` returns typed refusal; no second mutation |
| partial failure / timeout | retain evidence and enter reconciliation; do not claim success |
| stale / unavailable | later HEAD/foreign changes, conflict, missing Git, or external unknown yields refusal/environment fault 3 |

## 9. Tiny implementation steps
1. In `src/lib.rs`: Define `RollbackTarget`, `RollbackRequest`, `RollbackReceipt`, and error variants; `cargo check -p rollback` exits 0.
2. In `src/guard.rs`: Implement eligibility predicates including nonexistent-artifact and already-rolled-back checks; `cargo test -p rollback nonexistent_artifact_is_refused` passes.
3. In `src/guard.rs`: Add path-containment and current-HEAD/foreign-modification CAS guards; `cargo test -p rollback path_escape_is_refused` passes.
4. In `src/action.rs` and `src/reconcile.rs`: Implement private worktree cleanup port and post-action verification against a fresh temporary Git repository; `cargo test -p rollback private_rollback_emits_receipt` passes.
5. In `tests/rollback.rs`: Wire shared-ref compensating proposal and real CLI binary; assert one success, one refusal, and one reconciliation receipt; `cargo test -p rollback rollback_emits_requeue_edge_on_shared_ref` passes.

## 10. Test matrix
**Unit tests:** nonexistent artifact, unapplied artifact, second rollback, path escape, moved HEAD, foreign modification.

**Integration/contract tests:** post failure reaches rollback; private rollback emits requeue/repair edge; shared ref emits approval-bound compensation.

**Hidden tests:** symlinked worktree, scratch collision, later commit, conflict during revert, missing refusal receipt, external side effect present.

**Property tests:** no generated target outside owned root is accepted; 1,000 fixed-seed paths/refs.

**Differential tests:** existing `rollback_repo`/`rollback_artifact` fixtures versus new guard/action ports; allowed divergence is refusal strengthening for shared refs and zero coverage.

**Real-binary/effect test:** `target/debug/fleet rollback --artifact artifact-blueprint-rollback` against a fresh temporary repo; assert receipt, resulting HEAD/tree, and preserved artifact evidence.

### Named integration tests (these 3 must compile and pass)

| Test name (exact) | Inputs | Expected output | Mutation / anti-stub caught |
|---|---|---|---|
| `rollback::tests::nonexistent_artifact_is_refused` | `RollbackRequest` referencing an artifact ID that has no durable record in store | `Err(RollbackError::ArtifactNotFound)` with a receipt written | catches accepting any target unconditionally; absence of artifact evidence must refuse |
| `rollback::tests::path_escape_is_refused` | `RollbackRequest` with `RollbackTarget::OwnedWorktree` whose path traverses outside the `.worktrees` root (e.g. `../../sensitive`) | `Err(RollbackError::Containment)` with a receipt written | catches removing the path-containment check; arbitrary deletion of user data is the failure mode |
| `rollback::tests::private_rollback_emits_receipt` | `RollbackRequest` for a valid owned worktree path against a fresh temporary Git repo whose HEAD matches the applied commit | `RollbackReceipt { status: "rolled_back", checked: 1, total: 1 }` and the worktree path is removed | lld edge contract — confirms guard, action, and receipt are all wired; catches a no-op that drops the receipt |

## 11. Mutation targets and anti-stub proof

| Function mutated | Mutation applied | Test that catches it | Why it's the right catcher |
|---|---|---|---|
| `check_containment` in `src/guard.rs` | Remove path-containment predicate, accepting any `PathBuf` target | `rollback::tests::path_escape_is_refused` | Rollback cannot delete user data outside owned root; removing the check is the exact attack |
| `check_cas` in `src/guard.rs` | Skip current-HEAD comparison, always treating HEAD as matching applied commit | `rollback::tests::private_rollback_emits_receipt` with a post-apply HEAD change | No clobber of newer work; a moved HEAD must refuse |
| `check_already_rolled_back` in `src/guard.rs` | Remove terminal-status guard, allowing a second rollback attempt | `rollback::tests::nonexistent_artifact_is_refused` (second-call variant) | Action is one-use; a receipt with terminal status must refuse replay |
| `apply_shared_ref_action` in `src/action.rs` | Replace compensating-revert proposal with `git reset --hard` | `rollback::tests::rollback_emits_requeue_edge_on_shared_ref` | Compensation preserves history; reset destroys it and must be caught by the policy test |
| `write_receipt` in `src/receipt.rs` | Delete the artifact/evidence record after a failed revert | `rollback::tests::private_rollback_emits_receipt` (failure-path variant) | Failure remains auditable; suppressing the receipt conceals evidence |
| `verify_action` in `src/reconcile.rs` | Return `Ok(receipt)` even when the revert command exits non-zero | `rollback::tests::private_rollback_emits_receipt` (conflict-path variant) | No false recovery; an unconfirmed revert must enter reconciliation, not success |

Reviewer changes shared rollback to `reset --hard`; shared-ref test must fail and policy review must reject it.

Safety mutation floor: `caught/total >= 80%` for all authority predicates; reviewer manually enables one bypass mutation and confirms kill.

## 12. Verification recipe and denominators
```bash
cargo test -p rollback --no-fail-fast
cargo clippy -p rollback --all-targets -- -D warnings
find crates/rollback -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +
target/debug/fleet rollback --artifact artifact-blueprint-rollback
# Mutation floor: caught/total >= 80% (authority)
```
Expected: refusal/guard cases `checked=10,total=10`; property paths `checked=1000,total=1000`; fresh-repo effect `checked=1,total=1`; shared-ref compensation remains approval/provider proof gated.

## 13. Definition of done
All of the following must be true — each is checkable by inspection or command output, no subjective criteria:

- `cargo test -p rollback --no-fail-fast` exits 0 with `test result: ok` in output.
- `rollback::tests::nonexistent_artifact_is_refused` appears in test output and passes.
- `rollback::tests::path_escape_is_refused` appears in test output and passes.
- `rollback::tests::private_rollback_emits_receipt` appears in test output and passes.
- `rollback::tests::rollback_emits_requeue_edge_on_shared_ref` appears in test output and passes.
- `cargo clippy -p rollback --all-targets -- -D warnings` exits 0 (zero warnings).
- No source file under `crates/rollback/src/` exceeds 80 lines (`find crates/rollback/src -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +` exits 0).
- Mutation floor: `caught/total >= 80%` across guard and action targets.

## 14. Failure stories and review questions
**Failure → Cause → Fix:** rollback erased a later developer commit → it used reset without a HEAD guard → require exact applied HEAD and use compensating revert for shared refs.

1. What may be deleted? 2. When is rollback impossible? 3. Does failed cleanup preserve enough evidence to reconcile?
