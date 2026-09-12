# BLUEPRINT — `post`

## 1. Identity and LLD path
- Node id / label / tag: `post / Post-Merge Verify / deterministic`
- LLD authority: `docs/LLD/LLD.md §14, §18`, `docs/LLD/LLD-META-L8-ADDENDUM.md §H`, and `docs/LLD/lld-full-detail.architecture.json:components[id=post]`
- Why this node exists: re-run protected evidence against the exact integrated tree before approval.
- Incoming edges: `integrate -> post` (`IntegrationReceipt`, after HEAD, candidate/tree digest, acceptance digest).
- Outgoing edges: `post -> approval` (`PostVerdict` pass); `post -> rollback` (`PostVerdict` fail).
- Build status: `partial` — `crates/fleet-verify`, `src/dispatch/verify_cmd.rs`, and receipt paths exist, but invalidation-on-HEAD-change is not a complete node.

## 2. Responsibility and non-goals
**Owns:** exact-HEAD revalidation, affected gate execution, secret scan result intake, denominator publication, and pass/fail receipt.

**Does not own:** merging, fixing code, approval, external publication, or rollback mutation.

## 3. Boundary and authority
Controller-owned deterministic gate. It reads the integrated tree through a quarantined verifier and may write only a verdict/receipt. It requires a grant for local test execution, but that grant cannot authorize publication. Any HEAD change, tracked test mutation, missing acceptance, secret finding, zero-input gate, or unavailable tool blocks approval and emits a rollback edge.

## 4. Crate/package layout
```text
crates/post/
  Cargo.toml
  src/lib.rs                 # exports, ≤40 lines
  src/verify.rs              # HEAD/digest gate, ≤80 lines
  src/receipt.rs             # verdict receipt, ≤60 lines
  src/port.rs                # command/scanner ports, ≤60 lines
  tests/post.rs              # fresh-repo and fault cases, ≤80 lines
```

## 5. Public API contract
```rust
pub struct PostVerifyEvidence { pub integration_receipt_ref: String, pub gate_evidence_refs: Vec<String>, pub checked: u64, pub total: u64 }
pub struct PostVerifyFailure { pub integration_receipt_ref: String, pub reason: String, pub effect_state: String, pub checked: u64, pub total: u64 }
pub struct PostRequest { pub repo: PathBuf, pub expected_head: String, pub acceptance_digest: String, pub required_gates: Vec<GateSpec>, pub grant: Grant }
pub struct PostVerdict { pub status: Status, pub head: String, pub checked: u64, pub total: u64, pub evidence_digest: String }
pub trait GateRunner { fn run(&self, gate: &GateSpec, repo: &Path) -> Result<GateResult, PostError>; }
pub fn verify_after_merge(r: &impl GateRunner, req: PostRequest) -> Result<PostVerdict, PostError>;
```
Precondition: exact integrated HEAD and nonempty gate set. Postcondition: pass only when every gate ran and `0<checked==total`; no panic.

## 6. Data model, invariants, and failure taxonomy
| Type / record | Invariant | Illegal state prevented | Failure / exit code |
|---|---|---|---|
| post request | expected HEAD/tree/acceptance exact | stale evidence | `Stale/8` |
| gate result | gate examined input and publishes counts | vacuous green | `ZeroCoverage/6` |
| verdict | all required gates pass, no secret | unsafe approval | `GateFailed/6` |
| receipt | before/after head and evidence digest durable | unverifiable result | `Receipt/8` |

Commands run in a separate quarantined process; clock and scanner are ports, counts integers, hashes fixed strings.

## 7. Reuse map and adoption decisions
| Existing source or stable library | Evidence/version/commit/license | Use | Why not custom code | Proof still required |
|---|---|---|---|---|
| `src/dispatch/verify_cmd.rs:35-52` | local source seam | command/gate dispatch vocabulary | preserve CLI behavior | node contract |
| `crates/fleet-verify` | architecture/source package | deterministic checks and secret scan ports | no second verifier | exact-tree integration |
| `crates/fleet-merge/src/invariant.rs:18-35` | local exact source | moved-head/nonempty guard | preserve existing invariant | mutation test |
| Git CLI + existing verifier commands | runtime-discovered; versions unverified here | authoritative repository/test execution | avoid custom shell semantics | real binary with output |

No claim is made that a provider scanner detects every secret; false negatives remain an explicit limitation.

### Cargo.toml snippet (pinned)
```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
tokio = { version = "1", features = ["rt-multi-thread", "process", "macros"] }
```

## 8. Behavior matrix
### `verify_after_merge`
| Input/failure dimension | Observable behavior |
|---|---|
| empty / missing | no gates means refusal with receipt |
| wrong type / Unicode | malformed gate/refusal; paths are contained and text preserved |
| huge / negative | bounded logs/artifacts; negative counts refuse |
| duplicate / concurrent | same `(head,acceptance)` returns prior verdict; concurrent target verification is serialized |
| partial failure / timeout | failed gate and receipt; no approval edge; rollback is proposed, not hidden |
| stale / unavailable | changed HEAD or missing tool is stale/environment fault 3, never pass |

## 9. Tiny implementation steps
1. In `src/lib.rs`: define `PostRequest`, `PostVerdict`, `GateSpec`, `Status`, and `PostError` types → run `cargo check -p post`.
2. In `src/verify.rs`: add exact HEAD re-read and acceptance digest check blocking approval on any mismatch → add `post::tests::stale_head_refuses` and `post::tests::acceptance_digest_mismatch_refuses`; run `cargo test -p post verify`.
3. In `src/port.rs`: port quarantined `GateRunner` and zero-input denominator guard → add `post::tests::zero_gate_refuses` and `post::tests::gate_exit_three_is_environment_fault`; run `cargo test -p post port`.
4. In `src/receipt.rs`: write durable `PostVerdict` receipt on both pass and refusal → add `post::tests::refusal_still_writes_receipt`; run `cargo test -p post receipt`.
5. In `tests/post.rs`: run the real post-merge binary on a fresh fixture → `target/debug/fleet gate --id post-merge-blueprint-smoke` shows `checked=n,total=n` with `n>0`; both pass and fail produce receipts with the correct outgoing edge.

## 10. Test matrix
**Unit tests:** HEAD mismatch, zero gate, failing gate, secret finding, exact pass.

**Integration/contract tests:** real integrate receipt feeds post and pass feeds approval; fail feeds rollback.

**Hidden tests:** HEAD changes mid-run, test modifies tracked file, gate exits 3, fake empty success, acceptance digest mismatch.

**Property tests:** removing any gate cannot turn a failed verdict into pass; 500 fixed-seed gate sets.

**Differential tests:** existing `fleet-verify` fixture outcomes versus post adapter; only stronger stale/denominator refusal may differ.

**Real-binary/effect test:** `target/debug/fleet gate --id post-merge-blueprint-smoke` on a real integrated fixture; verify receipt and exit code.

### Named integration tests (these 3 must compile and pass)

| Test name (exact) | Inputs | Expected output | Mutation / anti-stub caught |
|---|---|---|---|
| `post::tests::stale_head_refuses` | `PostRequest` with `expected_head = "abc123"`; actual HEAD in the fixture repo advanced to `"def456"` before `verify_after_merge` runs | `Err(PostError::Stale)` | lld edge contract — confirms HEAD re-read is enforced; catches the skip-HEAD-reread mutant in `src/verify.rs` that uses the stale expected HEAD, allowing approval of superseded evidence |
| `post::tests::zero_gate_refuses` | `PostRequest` with `required_gates = vec![]` (empty gate list) | `Err(PostError::ZeroCoverage)` | Catches the 0/0-as-pass mutant in `src/publish.rs`; any implementation that accepts an empty gate set as vacuously green produces the wrong verdict |
| `post::tests::refusal_still_writes_receipt` | `PostRequest` that fails at the gate stage (injected failing `GateRunner`) | `PostVerdict` receipt written to store even though `status = Failed` | Catches the omit-receipt-on-refusal mutant in `src/receipt.rs`; removing the refusal receipt makes the verdict unauditable and breaks the durability assertion |

## 11. Mutation targets and anti-stub proof

| Function mutated | Mutation applied | Test that catches it | Why it's the right catcher |
|---|---|---|---|
| `verify_after_merge` in `src/verify.rs` | Skip HEAD re-read; use `expected_head` from request without re-reading the repo ref | `post::tests::stale_head_refuses` | Test advances HEAD after request construction; skipping the re-read allows stale evidence to produce a pass verdict, breaking the `Stale` error assertion |
| `run_gate` in `src/verify.rs` | Treat `GateResult { checked: 0, total: 0 }` as pass | `post::tests::zero_gate_refuses` | Test sends an empty gate set and asserts `ZeroCoverage`; the mutant returns a passing verdict, breaking the denominator assertion |
| `verify_after_merge` acceptance digest check in `src/verify.rs` | Skip acceptance digest comparison; accept any `acceptance_digest` string | `post::tests::acceptance_digest_mismatch_refuses` | Test mutates `acceptance_digest` in the request and asserts `Stale`; removing the check lets tampered acceptance pass, breaking the integrity assertion |
| `run_gate` in `src/verify.rs` | Map gate exit code 3 to `GateResult::Pass` instead of `PostError::EnvironmentFault` | `post::tests::gate_exit_three_is_environment_fault` | Test runs a gate that exits 3 and asserts environment fault; the mutant converts it to pass, hiding a missing tool as a successful gate |
| `write_verdict` in `src/receipt.rs` | Omit receipt write on refusal; only write when `status = Pass` | `post::tests::refusal_still_writes_receipt` | Test expects a receipt even when the gate fails; the mutant omits it, making the refusal unauditable and breaking the durability assertion |

Reviewer disables one required gate; all-gates test must fail.

Safety mutation floor: `caught/total >= 80%`.

## 12. Verification recipe and denominators
```bash
cargo test -p post --no-fail-fast
cargo clippy -p post --all-targets -- -D warnings
find crates/post -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +
target/debug/fleet gate --id post-merge-blueprint-smoke
# Mutation floor: caught/total >= 80% (authority)
```
Expected: gate matrix `checked=n,total=n`, `n>0`, and `checked==total`; property `checked=500,total=500`; real binary has one receipt for each pass/fail case. Do not call local success remote CI or production proof.

## 13. Definition of done
All of the following must be true — each is checkable by command output, no subjective criteria:

- `cargo test -p post --no-fail-fast` exits 0 with `test result: ok` in output.
- `post::tests::zero_gate_refuses` appears in test output and passes.
- `cargo clippy -p post --all-targets -- -D warnings` exits 0.
- `find crates/post -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +` exits 0.
- `target/debug/fleet gate --id post-merge-blueprint-smoke` output contains `checked=n,total=n` with `n>0`.
- Mutation floor: `caught/total >= 80%` for all authority predicates in §11; disabling one required gate causes `post::tests::all_gates_required` to fail.

## 14. Failure stories and review questions
**Failure → Cause → Fix:** approval was requested from tests run on a newer HEAD → HEAD was not re-read → invalidate immediately on any ref change.

1. What exact tree did the gates inspect? 2. What happens if a tool is missing? 3. Does failure reach rollback with evidence?
