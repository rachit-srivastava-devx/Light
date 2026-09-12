# BLUEPRINT — `integrate`

## 1. Identity and LLD path
- Node id / label / tag: `integrate / Integration Merge / deterministic`
- LLD authority: `docs/LLD/LLD.md §14, §15`, `docs/LLD/LLD-META-L8-ADDENDUM.md §C`, and `docs/LLD/lld-full-detail.architecture.json:components[id=integrate]`
- Why this node exists: merge one frozen, verified candidate into a controller-owned local ref using CAS on expected HEAD.
- Incoming edges: `verify -> integrate` (`pass`, frozen tree, candidate digest, base HEAD, grants, acceptance digest).
- Outgoing edges: `integrate -> post` (`IntegrationReceipt` with merge commit and before/after refs).
- Build status: `partial` — `crates/fleet-merge/src/merge.rs:14-153`, `lane_manager.rs:139-159`, `worktree.rs`, and `invariant.rs` are seams; the complete LLD CAS/receipt path is not proven.

## 2. Responsibility and non-goals
**Owns:** integration serialization, worktree containment, expected-HEAD CAS, merge preflight, merge receipt, and cleanup after durable receipt.

**Does not own:** source edits, acceptance tests, post-merge verification, publication, or automatic conflict guessing.

## 3. Boundary and authority
Parent-only effect boundary. Workers return fd-3 observations and never receive ledger/state/socket paths or merge credentials. Integration requires a capability grant covering the local target ref and exact candidate digest. It writes only inside `.worktrees/<run>` and the local integration ref; a conflict, moved HEAD, dirty foreign file, or changed candidate becomes typed refusal with a receipt.

## 4. Crate/package layout
```text
crates/integrate/
  Cargo.toml
  src/lib.rs                 # exports, ≤40 lines
  src/cas.rs                 # expected HEAD predicate, ≤60 lines
  src/merge.rs               # git command port, ≤80 lines
  src/receipt.rs             # integration receipt, ≤60 lines
  src/cleanup.rs             # owned worktree cleanup, ≤60 lines
  tests/integration.rs       # fresh-repo real-git tests, ≤80 lines
```

## 5. Public API contract
```rust
pub struct MergeRequest { pub repo: PathBuf, pub expected_head: String, pub lane_head: String, pub candidate_digest: String, pub grant: Grant }
pub struct IntegrationReceipt { pub before: String, pub after: String, pub lane: String, pub candidate_digest: String, pub checked: u64, pub total: u64 }
pub struct ChangeSet { pub id: String, pub run_id: String, pub repo_ids: Vec<String>, pub expected_base_heads: Vec<String>, pub interface_versions: Vec<String>, pub dependency_digest: String, pub operation_key: String, pub grant_id: String, pub state: SagaState, pub revision: u64 }
pub struct RepoOperation { pub changeset_id: String, pub repo_id: String, pub operation_key: String, pub base_head: String, pub candidate_digest: String, pub expected_resource_version: Option<String>, pub remote_ref: Option<String>, pub state: String, pub result_digest: Option<String> }
pub enum SagaState { Preparing, Verified, Publishing, Partial, Complete, Compensating }
pub trait GitPort { fn merge_tree(&self, req: &MergeRequest) -> Result<(), IntegrateError>; fn merge(&self, req: &MergeRequest) -> Result<String, IntegrateError>; }
pub fn integrate(git: &impl GitPort, req: MergeRequest) -> Result<IntegrationReceipt, IntegrateError>;
```
Preconditions: repo is contained, grant matches ref/digest, and `0<checked==total` evidence
exists in the request. Multi-repository work persists one `ChangeSet` and one `RepoOperation` per
repository and follows Addendum §C. Postcondition: receipt is durable before cleanup; no panic.

## 6. Data model, invariants, and failure taxonomy
| Type / record | Invariant | Illegal state prevented | Failure / exit code |
|---|---|---|---|
| `MergeRequest` | expected HEAD and candidate digest exact | stale candidate merge | `CasMismatch/8` |
| worktree | path under repo `.worktrees`, unique via `mktemp`/TempDir | path escape/deletion collision | `Containment/6` |
| grant | target ref, scope, expiry, effect class match | unauthorized merge | `Grant/7` |
| receipt | before, after, lane, digest, `checked,total` | untraceable mutation | `Receipt/8` |

All Git and clock operations are ports. Use parent lock/single writer; do not adopt `git2` for mutation. Hashes are fixed strings, counts integers.

## 7. Reuse map and adoption decisions
| Existing source or stable library | Evidence/version/commit/license | Use | Why not custom code | Proof still required |
|---|---|---|---|---|
| `crates/fleet-merge/src/merge.rs:14-78` | exact local source seam | merge outcome and command boundary | preserve existing refusal vocabulary | CAS/receipt contract |
| `crates/fleet-merge/src/worktree.rs:13-62` | exact local source seam | worktree create/remove | centralize containment | owned-path test |
| `crates/fleet-merge/src/invariant.rs:7-35` | exact local source seam | nonempty/head/file predicates | avoid duplicate guards | mutation tests |
| Git CLI, current host `git version` | primary Git docs; version is runtime-discovered, not pinned here | authoritative worktree/merge | `git2` would create a second mutation engine | fresh-repo real-git smoke |

Research records Git v2.51.0 as current candidate, but local version must be recorded at implementation time; this blueprint does not claim provider/version adoption.

### Cargo.toml snippet (pinned)
```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
tokio = { version = "1", features = ["rt-multi-thread", "process", "macros"] }
```

## 8. Behavior matrix
### `integrate`
| Input/failure dimension | Observable behavior |
|---|---|
| empty / missing | missing candidate/grant/ref refuses with receipt |
| wrong type / Unicode | malformed ref/digest refuses; branch names are validated, Unicode IDs remain data |
| huge / negative | bounded output/paths; no negative counts; merge diff size cap refuses |
| duplicate / concurrent | same operation key returns prior receipt; lock serializes target ref |
| partial failure / timeout | merge worktree retained for reconciliation; no success receipt |
| stale / unavailable | moved HEAD, dirty foreign files, conflict, or missing Git is typed refusal/environment fault 3 |

## 9. Tiny implementation steps
1. In `src/lib.rs`: define `MergeRequest`, `IntegrationReceipt`, `Grant`, and `IntegrateError` types → run `cargo check -p integrate`.
2. In `src/cas.rs`: port expected-HEAD CAS predicate and worktree containment guard → add `integrate::tests::moved_head_refuses` and `integrate::tests::path_outside_worktrees_refuses`; run `cargo test -p integrate cas`.
3. In `src/merge.rs`: add `git merge-tree --write-tree` port with typed conflict detection → add `integrate::tests::conflict_produces_refusal`; run `cargo test -p integrate merge`.
4. In `src/receipt.rs`: write durable `IntegrationReceipt` after parent-owned `git merge --no-ff`, before any cleanup → add `integrate::tests::receipt_is_written_before_cleanup`; run `cargo test -p integrate --test integration`.
5. In `src/cleanup.rs`: add idempotent owned worktree cleanup after receipt → add `integrate::tests::cleanup_is_idempotent`; run `cargo test -p integrate cleanup` then `target/debug/fleet pipeline_probe --real-git` shows `checked=1,total=1`.

## 10. Test matrix
**Unit tests:** expected-head equality, grant scope/expiry, worktree containment, nonempty diff.

**Integration/contract tests:** verify pass reaches integrate and post receives exact after commit/digest.

**Hidden tests:** parent moved between preflight and merge, branch already checked out, symlink escape, stale worker response, cleanup before receipt.

**Property tests:** generated paths never escape the resolved `.worktrees` root; 1,000 fixed-seed cases.

**Differential tests:** `fleet-merge` outcomes versus new node for existing merge fixtures; only explicit receipt/CAS strengthening may differ.

**Real-binary/effect test:** a fresh temporary Git repository runs `git merge-tree`, merge, receipt, and cleanup; `target/debug/fleet pipeline_probe` must show `checked=1,total=1`.

### Named integration tests (these 3 must compile and pass)

| Test name (exact) | Inputs | Expected output | Mutation / anti-stub caught |
|---|---|---|---|
| `integrate::tests::moved_head_refuses` | `MergeRequest` with `expected_head = "abc123"` and current HEAD advanced to `"def456"` via a real-git fixture | `Err(IntegrateError::CasMismatch)` | lld edge contract — confirms the CAS predicate is enforced; catches the unconditional-true mutant in `src/cas.rs` that merges regardless of HEAD position |
| `integrate::tests::conflict_produces_refusal` | `MergeRequest` pointing to a branch with a genuine merge conflict in a fresh temporary repository | `Err(IntegrateError::Conflict)` | Catches removal of conflict detection in `src/merge.rs`; without detection the merge proceeds and returns `Ok`, breaking the refusal assertion |
| `integrate::tests::receipt_is_written_before_cleanup` | `MergeRequest` that succeeds; injected failure simulated after merge but before receipt write | `IntegrationReceipt` present in store; no cleanup executed before receipt | Catches cleanup-before-receipt mutant in `src/receipt.rs`; moving cleanup before the write loses the receipt on injected failure, breaking the durability assertion |

## 11. Mutation targets and anti-stub proof

| Function mutated | Mutation applied | Test that catches it | Why it's the right catcher |
|---|---|---|---|
| `cas_predicate` in `src/cas.rs` | Change CAS to unconditional true; never compare `expected_head` with current HEAD | `integrate::tests::moved_head_refuses` | Test advances HEAD before the merge; unconditional-true CAS merges a stale candidate and the `CasMismatch` assertion fails |
| `merge_tree` in `src/merge.rs` | Skip conflict detection; return `Ok(())` regardless of exit code | `integrate::tests::conflict_produces_refusal` | Test uses a real conflict fixture; removing detection maps the non-zero exit to success, breaking the `Conflict` variant assertion |
| `write_receipt` in `src/receipt.rs` | Move worktree cleanup before receipt write; on injected failure the receipt is lost | `integrate::tests::receipt_is_written_before_cleanup` | Test injects a failure post-merge; the mutant's early cleanup discards evidence before the durability assertion runs |
| `path_containment_check` in `src/cas.rs` | Accept paths outside `.worktrees`; remove canonical-path prefix guard | `integrate::tests::path_outside_worktrees_refuses` | Test sends an escaping path; removing the guard lets unauthorized cleanup proceed, breaking the `Containment` error assertion |
| `write_receipt` candidate digest in `src/receipt.rs` | Omit `candidate_digest` field in `IntegrationReceipt` | `integrate::tests::receipt_is_written_before_cleanup` + digest unit test | Edge contract requires non-empty digest binding; empty digest breaks the immutability assertion in the contract test |

Reviewer manually changes CAS to unconditional true; moved-head test must fail.

Safety mutation floor: `caught/total >= 80%`.

## 12. Verification recipe and denominators
```bash
cargo test -p integrate --no-fail-fast
cargo clippy -p integrate --all-targets -- -D warnings
find crates/integrate -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +
git version
target/debug/fleet pipeline_probe --real-git
# Mutation floor: caught/total >= 80% (authority)
```
Expected: merge fixtures `checked=6,total=6`; property paths `checked=1000,total=1000`; real repository `checked=1,total=1`; no remote publication is performed.

## 13. Definition of done
All of the following must be true — each is checkable by command output, no subjective criteria:

- `cargo test -p integrate --no-fail-fast` exits 0 with `test result: ok` in output.
- `integrate::tests::moved_head_refuses` appears in test output and passes.
- `cargo clippy -p integrate --all-targets -- -D warnings` exits 0.
- `find crates/integrate -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +` exits 0.
- `target/debug/fleet pipeline_probe --real-git` output contains `checked=1,total=1`.
- Mutation floor: `caught/total >= 80%` for all authority predicates in §11; manually changing CAS to unconditional true causes `integrate::tests::moved_head_refuses` to fail.

## 14. Failure stories and review questions
**Failure → Cause → Fix:** a lane merged after the parent advanced → CAS was checked only before scheduling → re-read HEAD immediately before locked merge and refuse on change.

1. What exact ref can this grant mutate? 2. Is receipt durable before cleanup? 3. Can a worker perform the merge?
