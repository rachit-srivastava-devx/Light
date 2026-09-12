# BLUEPRINT — `builder`

## 1. Identity and LLD path
- Node id / label / tag: `builder / Builder Agent / model,gated`
- LLD authority: `docs/LLD/LLD.md §3, §12, §14` and `docs/LLD/lld-full-detail.architecture.json:components[id=builder]`
- Why this node exists: apply an accepted module contract in an isolated private worktree.
- Incoming edges: `ready -> builder` (`lease + module`), `context -> builder` (`compiled manifest`).
- Outgoing edges: `builder -> review` (`candidate observation`).
- Build status: `partial` — `crates/fleet-worker`/`fleet-crew` provide adapter seams; no complete LLD builder authority.

## 2. Responsibility and non-goals
**Owns:** lease-bound subprocess/session execution, candidate tree capture, fd-3 result normalization, cancellation and unexpected-file report.
**Does not own:** grants, ledger/state/socket paths, acceptance tests, review, verification, merge, or publication.

## 3. Boundary and authority
Gated model worker in a private worktree. Parent stamps actor/time/resolved model and owns ledger/state/socket. Worker receives only lease, context, tools and fd 3; no ledger path, state dir, socket, credential, approval, or budget authority. ACP is a proposed first adapter; capability, login, sandbox, and quality remain separately unverified.

## 4. Crate/package layout
```text
crates/builder/
  Cargo.toml
  src/lib.rs             # ≤40 lines
  src/lease.rs           # ≤80 lines
  src/launch.rs          # ≤80 lines
  src/result.rs          # ≤80 lines
  tests/real_binary.rs   # ≤80 lines
```

## 5. Public API contract
```rust
pub trait WorkerLauncher: Send + Sync { fn run(&self, lease: &Lease, ctx: &ContextManifest, fd3: i32) -> Result<Observation, BuilderError>; }
pub fn validate_observation(o: &Observation, lease: &Lease) -> Result<(), BuilderError>;
pub struct Lease { pub base: String, pub plan_digest: String, pub write_scope: Vec<String>, pub generation: u64 }
pub struct Observation { pub tree_digest: String, pub changed: Vec<String>, pub unexpected: Vec<String>, pub exit_code: i32, pub fd3_digest: String }
```
Precondition: private worktree/base/scope/digest exist. Postcondition: only scoped files changed and observation binds exact tree/lease. No fake child is sufficient proof.

## 6. Data model, invariants, and failure taxonomy
| Type / record | Invariant | Illegal state prevented | Failure / exit code |
|---|---|---|---|
| lease | generation/base/scope immutable | stale worker mutation | `StaleLease` / 8 |
| candidate | tree digest and changed paths recorded | unreviewed tree | `ScopeViolation` / 6 |
| fd-3 result | framed, typed, complete | stdout prose as result | `Protocol` / 8 |
| cancellation | descendants reaped, receipt written | orphan process/effect | `Cancelled` / 7 |

## 7. Reuse map and adoption decisions
| Existing source or stable library | Evidence/version/commit/license | Use | Why not custom code | Proof still required |
|---|---|---|---|---|
| `crates/fleet-worker/src/spawn/fd3.rs:27-30` | graph exact source | fd-3 parent/child channel seam | retain protocol boundary | real child framing smoke |
| `crates/fleet-worker` spawn/reap | local source/tests | process lifecycle | no custom reaper | descendant cleanup |
| Tokio 1.53.1, MIT; local | research ledger | async process/timeout | maintained primitive | real timeout test |
| ACP Rust 2.1.0, Apache-2.0; documented only | research ledger | first adapter experiment | provider protocol adoption | pinned checkout/conformance/login smoke |

No custom implementation is introduced where the listed crate already provides the facility.

### Cargo.toml snippet (pinned)
```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
tokio = { version = "1", features = ["rt-multi-thread", "process", "macros"] }
```

## 8. Behavior matrix
### `WorkerLauncher::run`
| Input/failure dimension | Observable behavior |
|---|---|
| empty / missing | lease/context/fd refusal before spawn |
| wrong type / Unicode | typed protocol error; paths canonicalized safely |
| huge / negative | scope/output limits; invalid generation refuses |
| duplicate / concurrent | generation fence rejects stale duplicate; one lease owner |
| partial failure / timeout | kill descendants, freeze candidate, receipt, no success |
| stale / unavailable | no spawn or typed environment fault; never agent failure |

## 9. Tiny implementation steps
1. In `src/lib.rs` and `src/lease.rs`: define `Lease`, `Observation`, and `BuilderError` types → `cargo check -p builder` exits 0.
2. In `src/lease.rs`: add scope and generation validator that rejects out-of-scope paths and stale generations → add `builder::tests::scope_violation_refused` and run `cargo test -p builder scope_violation_refused` exits 0.
3. In `src/result.rs`: wire fd-3 framing for typed result; reject malformed or truncated frames → add `builder::tests::fd3_frame_required_for_success` and run `cargo test -p builder fd3_frame_required_for_success` exits 0.
4. In `src/launch.rs`: add real subprocess spawn, timeout, and descendant reap via Tokio → add `builder::tests::timeout_reaps_descendants` and run `cargo test -p builder timeout_reaps_descendants` exits 0.
5. In `tests/real_binary.rs`: invoke `env!("CARGO_BIN_EXE_fleet")` worker → `target/debug/fleet build --fixture tests/fixtures/blueprint-builder/scope-limited.json --json` output contains `checked=N,total=N` with N>0; candidate digest and receipt appear in output.

## 10. Test matrix
**Unit:** scope, generation, framing, exit-code mapping.
**Integration/contract:** ready lease → real worker → review observation.
**Hidden:** worker tries ledger/state/socket env, edits acceptance test, writes outside scope, hangs child.
**Property:** frame parser round-trips bounded records; fixed seed 200 cases.
**Differential:** candidate manifest versus Git diff canonicalization.
**Real-binary:** `env!("CARGO_BIN_EXE_fleet")` worker invocation; fake child may supplement, never replace.

### Named integration tests (these 3 must compile and pass)

| Test name (exact) | Inputs | Expected output | Mutation / anti-stub caught |
|---|---|---|---|
| `builder::tests::scope_violation_refused` | `Lease` with `write_scope = ["src/"]`; worker `Observation` listing a file outside scope (`tests/acceptance.rs`) | `Err(BuilderError::ScopeViolation)` and no candidate emitted | catches scope-validator removal — any implementation that accepts out-of-scope writes passes them to review |
| `builder::tests::fd3_frame_required_for_success` | worker subprocess that exits 0 but writes nothing to fd 3 | `Err(BuilderError::Protocol)` — success requires a well-formed fd-3 frame | catches empty-fd3 pass-through; a stub treating exit-0 as success fails here |
| `builder::tests::empty_write_set_refused` | `Observation` with `changed = []` submitted as a candidate | `Err(BuilderError::ScopeViolation)` — an empty write set is not a valid candidate | catches no-op worker stub; a stub returning an empty `Observation` as success fails here |

## 11. Mutation targets and anti-stub proof

| Function mutated | Mutation applied | Test that catches it | Why it's the right catcher |
|---|---|---|---|
| `validate_scope` in `src/lease.rs` | Remove the out-of-scope path check — accept any file path in the observation | `builder::tests::scope_violation_refused` | test submits a path outside `write_scope`; removing the check lets it pass, breaking the `ScopeViolation` assertion |
| `parse_fd3_frame` in `src/result.rs` | Accept an empty or missing fd-3 frame as a valid result | `builder::tests::fd3_frame_required_for_success` | test spawns a child that writes nothing to fd 3; accepting empty frames makes the function return `Ok`, which the test asserts must not happen |
| `validate_scope` in `src/lease.rs` | Allow an empty `changed` set to pass as a valid observation | `builder::tests::empty_write_set_refused` | test provides `changed = []`; removing the emptiness guard lets the no-op observation pass and the refusal assertion fails |
| `validate_scope` in `src/lease.rs` | Accept env var `LEDGER_PATH` passed through to the worker subprocess | hidden env test | worker isolation — any observation that reads ledger state violates the worker boundary |

Safety mutation floor: `caught/total >= 75%`. Reviewer manually injects a fake child success and confirms the real-binary test goes red.

## 12. Verification recipe and denominators
```bash
cargo test -p builder --no-fail-fast
cargo clippy -p builder --all-targets -- -D warnings
find crates/builder -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +
target/debug/fleet build --fixture tests/fixtures/blueprint-builder/scope-limited.json --json
```
Report workers `checked=n,total=n,n>0`, changed/unexpected files, fd-3 frames, exit code, tree digest. ACP/sandbox/provider proof is currently unverified.

## 13. Definition of done
All of the following must be true — each is checkable by command output, no subjective criteria:

- `cargo test -p builder --no-fail-fast` exits 0 with `test result: ok` in output.
- `builder::tests::scope_violation_refused` appears in test output and passes.
- `builder::tests::fd3_frame_required_for_success` appears in test output and passes.
- `cargo clippy -p builder --all-targets -- -D warnings` exits 0 (zero warnings).
- `find crates/builder -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +` exits 0.
- `target/debug/fleet build --fixture tests/fixtures/blueprint-builder/scope-limited.json --json` output contains `checked=N,total=N` with `N>0`.
- Mutation floor: `caught/total >= 80%` for the 3 named mutation targets in §11 (out-of-scope file, ledger env, empty fd-3).
- `Cargo.toml` pins `serde`, `thiserror`, and `tokio` exactly as shown in §7.

## 14. Failure stories and review questions
**Failure → Cause → Fix:** worker reports success while parent cannot audit it → result used stdout and worker-owned paths → only fd 3 plus parent-stamped receipt is authoritative.
1. Can a fake child be the only proof? 2. What happens to descendants? 3. Can worker authorize an effect?
