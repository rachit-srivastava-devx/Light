# BLUEPRINT — `review`

## 1. Identity and LLD path
- Node id / label / tag: `review / Code Review / model,gated`
- LLD authority: `docs/LLD/LLD.md §14, §18` and `docs/LLD/lld-full-detail.architecture.json:components[id=review]`
- Why this node exists: independently inspect the frozen candidate before deterministic verification.
- Incoming edges: `builder -> review` (`candidate observation`), `context -> review` (`compiled manifest`).
- Outgoing edges: `review -> verify` (`reviewed candidate`).
- Build status: `partial` — `src/dispatch/adjudicate_cmd.rs`, verifier routes, and `fleet-plan` review contracts exist; not this node.

## 2. Responsibility and non-goals
**Owns:** independent findings, severity, diff/plan/evidence digest binding, and review decision.
**Does not own:** code edits, tests, secret scan, merge, or publication.

## 3. Boundary and authority
Gated model critique plus deterministic contract checks. Reviewer reads frozen tree and protected acceptance; it cannot modify either. A qualified different actor/model is required; findings are evidence, not sole security proof. Reject/timeout blocks verify and writes a receipt.

## 4. Crate/package layout
```text
crates/review/
  Cargo.toml
  src/lib.rs            # ≤40 lines
  src/findings.rs       # ≤80 lines — run_semgrep, run_ruff, apply_scope_filter live here
  src/decision.rs       # ≤80 lines — assemble_reviewed_candidate lives here
  tests/contract.rs     # ≤80 lines
```

### Cargo.toml (exact snippet — do NOT add semgrep or ruff as crate deps)
```toml
[package]
name = "review"
version = "0.1.0"
edition = "2021"

[dependencies]
serde       = { version = "1", features = ["derive"] }
serde_json  = "1"
thiserror   = "2"
tokio       = { version = "1", features = ["rt-multi-thread", "process", "macros"] }

# fleet-internal
fleet-plan  = { path = "../../crates/fleet-plan" }
```

Semgrep and Ruff are **external binaries** — they are not Rust crates and must not appear in `[dependencies]`. Install them separately (`pip install semgrep ruff` or via system package manager).

## 5. Public API contract
```rust
pub trait Reviewer: Send + Sync { fn inspect(&self, input: &ReviewInput) -> Result<ReviewResult, ReviewError>; }
pub struct ReviewInput { pub tree_digest: String, pub plan_digest: String, pub acceptance_digest: String, pub worker_id: String, pub reviewer_id: String }
pub struct ReviewResult { pub decision: Decision, pub findings: Vec<Finding>, pub checked: u64, pub total: u64, pub input_digest: String, pub output_digest: String }
```
Accept requires independent identity, exact digests, nonzero complete checks, and no blocking finding. Errors typed, no panic.

## 6. Data model, invariants, and failure taxonomy
| Type / record | Invariant | Illegal state prevented | Failure / exit code |
|---|---|---|---|
| finding | severity/path/rationale | prose-only review | `Malformed` / 6 |
| result | candidate/plan/acceptance digests exact | stale review | `Mismatch` / 8 |
| reviewer | worker != reviewer and cohort qualified | self-approval | `NotIndependent` / 6 |
| coverage | checked=total>0 | empty review | `ZeroCoverage` / 6 |

## 7. Reuse map and adoption decisions
| Existing source or stable library | Evidence/version/commit/license | Use | Why not custom code | Proof still required |
|---|---|---|---|---|
| `crates/fleet-plan/src/review/contract.rs:30-50` | exact source | role/cycle validation | retain established contract | new tree digest contract |
| `serde = { version = "1", features = ["derive"] }` | crates.io, MIT/Apache-2.0 | serialise ReviewInput/ReviewResult | standard | — |
| `serde_json = "1"` | crates.io, MIT/Apache-2.0 | parse Semgrep/Ruff JSON output | standard | — |
| `thiserror = "2"` | crates.io, MIT/Apache-2.0 | typed ReviewError variants | standard | — |
| `tokio = { version = "1", features = ["rt-multi-thread", "process", "macros"] }` | crates.io, MIT | async runtime + `tokio::process::Command` for binary invocations | standard | — |
| **Semgrep 1.177.0** (external binary, LGPL-2.1) | quality ledger — NOT a Rust crate | static supplementary findings | maintained pattern engine | pinned rules; scanned-file denominator > 0 |
| **Ruff 0.16.7** (external binary, MIT) | quality ledger — NOT a Rust crate | Python glue lint | no custom linter | repo environment version |

### Invoking external binaries via `std::process::Command`

Both Semgrep and Ruff are invoked as child processes. Neither appears in `Cargo.toml`.

```rust
// src/findings.rs — run_semgrep
pub fn run_semgrep(path: &str) -> Result<Vec<Finding>, ReviewError> {
    let output = std::process::Command::new("semgrep")
        .args(["scan", "--json", "--config", "auto", path])
        .output()
        .map_err(|e| ReviewError::ToolUnavailable(format!("semgrep: {e}")))?;
    let raw: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| ReviewError::Malformed(format!("semgrep json: {e}")))?;
    parse_semgrep_findings(&raw)
}

// src/findings.rs — run_ruff
pub fn run_ruff(path: &str) -> Result<Vec<Finding>, ReviewError> {
    let output = std::process::Command::new("ruff")
        .args(["check", "--output-format", "json", path])
        .output()
        .map_err(|e| ReviewError::ToolUnavailable(format!("ruff: {e}")))?;
    let raw: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| ReviewError::Malformed(format!("ruff json: {e}")))?;
    parse_ruff_findings(&raw)
}
```

In tests, replace the binary with a mock by injecting a `FindingsProvider` trait instead of calling `run_semgrep` directly — see test matrix §10.

## 8. Behavior matrix
### `Reviewer::inspect`
| Input/failure dimension | Observable behavior |
|---|---|
| empty / missing | refusal; no accept |
| wrong type / Unicode | typed input/finding error |
| huge / negative | bounded diff/findings; invalid count refusal |
| duplicate / concurrent | same frozen digest idempotent; changed tree conflicts |
| partial failure / timeout | blocked verify and receipt |
| stale / unavailable | mismatch/refusal, never pass |

## 9. Tiny implementation steps
1. In `src/lib.rs`: Define `ReviewedCandidate`, `Finding`, and error types; re-export all public items; `cargo check -p review` exits 0.
2. In `src/decision.rs`: Implement `assemble_reviewed_candidate` with self-review guard and `input_digest`/`output_digest` assertions; `cargo test -p review semgrep_finding_marks_review_failed` passes.
3. In `src/findings.rs`: Implement `run_semgrep`, `run_ruff`, and `apply_scope_filter` as distinct named functions; `cargo test -p review context_manifest_scopes_review_evidence` passes.
4. In `src/findings.rs`: Add model findings parser handling malformed output and timeout with typed errors; `cargo test -p review candidate_observation_triggers_code_review` passes.
5. In `tests/contract.rs`: Wire all three named integration tests end-to-end using `MockSemgrep`; `cargo test -p review candidate_observation_triggers_code_review semgrep_finding_marks_review_failed context_manifest_scopes_review_evidence` all pass.

## 10. Test matrix
**Unit:** self-review, changed tree, blocking finding, exact denominator.
**Integration/contract:** builder observation → review → verify payload.
**Hidden:** reviewer ignores acceptance, scanner sees zero files, model returns constant accept.
**Property:** any digest mutation rejects; fixed seed 200 cases.
**Differential:** existing review contract decisions for overlapping lifecycle states.
**Real-binary:** `target/debug/fleet review --fixture`; static tool capability is not production proof.

### Named integration tests (these three must compile and pass)

| Test name (exact) | Inputs | Expected output | Mutation / anti-stub caught |
|---|---|---|---|
| `review::tests::candidate_observation_triggers_code_review` | `CandidateObservation` loaded from fixture `tests/fixtures/blueprint-review/immutable-diff.json`; mock `FindingsProvider` returns one finding | `ReviewedCandidate` where `result.findings.len() >= 1` and both `input_digest` and `output_digest` are non-empty | lld edge contract — confirms the builder→review edge is wired and the node emits a structured `ReviewedCandidate`; catches a no-op handler that drops the observation |
| `review::tests::semgrep_finding_marks_review_failed` | Same fixture diff; `FindingsProvider` mock returns one finding with `severity = "WARNING"` | `ReviewedCandidate` where `passed == false` | anti-stub: any implementation that always returns `passed = true` fails here; catches `assemble_reviewed_candidate` returning a hard-coded pass |
| `review::tests::context_manifest_scopes_review_evidence` | `ContextManifest { scope: ["src/findings.rs"] }` + fixture diff whose changed paths include both `src/findings.rs` and `src/decision.rs`; `FindingsProvider` mock returns one finding per file | `ReviewedCandidate` where every `finding.path` is `"src/findings.rs"` — no finding for `src/decision.rs` | lld edge contract — confirms `apply_scope_filter` is enforced; catches removal of the scope filter |

**How to write the `FindingsProvider` mock** (so real Semgrep binary is not required in CI):
```rust
// tests/contract.rs
trait FindingsProvider: Send + Sync {
    fn findings(&self, path: &str) -> Vec<Finding>;
}

struct MockSemgrep { findings: Vec<Finding> }
impl FindingsProvider for MockSemgrep {
    fn findings(&self, _path: &str) -> Vec<Finding> { self.findings.clone() }
}
```
Inject via `Reviewer::with_provider(Box<dyn FindingsProvider>)`; the default production impl calls `run_semgrep`.

## 11. Mutation targets and anti-stub proof

| Function mutated | Mutation applied | Test that catches it | Why it's the right catcher |
|---|---|---|---|
| `run_semgrep` in `src/findings.rs` | Return `vec![]` (no findings regardless of input) | `review::tests::semgrep_finding_marks_review_failed` | Test injects a mock that returns a finding, then asserts `passed == false`; empty-findings mutation makes `assemble_reviewed_candidate` see no blocking finding and wrongly pass |
| `apply_scope_filter` in `src/findings.rs` | Return all files regardless of scope — remove the filter entirely | `review::tests::context_manifest_scopes_review_evidence` | Test sends a `ContextManifest` scoped to `src/findings.rs`; removing the filter leaks `src/decision.rs` findings into the output, breaking the assertion on `finding.path` |
| `assemble_reviewed_candidate` in `src/decision.rs` | Hard-code `passed: true` regardless of findings | `review::tests::semgrep_finding_marks_review_failed` | Test expects `passed == false` when a WARNING finding is present; the mutant's constant-true return is caught immediately |
| `populate_output_digest` in `src/decision.rs` | Leave `output_digest` empty in `ReviewResult` | `review::tests::candidate_observation_triggers_code_review` + digest unit test | Edge contract requires non-empty digests; protected acceptance must be bound |
| `check_reviewer_independence` in `src/decision.rs` | Skip `worker_id != reviewer_id` check | `review::tests::candidate_observation_triggers_code_review` (identity variant) | Independence invariant; self-approval is the attack |

Safety mutation floor: `caught/total >= 80%`. An independent reviewer must manually flip the blocking-finding branch and confirm the test goes red.

## 12. Verification recipe and denominators
```bash
cargo test -p review --no-fail-fast
cargo clippy -p review --all-targets -- -D warnings
target/debug/fleet review --fixture tests/fixtures/blueprint-review/immutable-diff.json --json
semgrep scan --config crates/review/assets/semgrep.yml --error --exclude target .
```
Publish files/findings `checked=n,total=n,n>0`, model/provider/prompt/input/output digests, and tool version. Current Semgrep/Gemini full execution is unverified.

## 13. Definition of done

All items are mechanically checkable. "Passes" means exit code 0 unless stated otherwise.

| # | Check | Command / assertion | Exits |
|---|---|---|---|
| 1 | All tests pass | `cargo test -p review --no-fail-fast` | 0 |
| 2 | Three named integration tests compile and pass verbatim | `cargo test -p review candidate_observation_triggers_code_review semgrep_finding_marks_review_failed context_manifest_scopes_review_evidence` | 0 |
| 3 | No clippy warnings | `cargo clippy -p review --all-targets -- -D warnings` | 0 |
| 4 | Real-binary fixture round-trip | `target/debug/fleet review --fixture tests/fixtures/blueprint-review/immutable-diff.json --json \| python3 -c "import json,sys; d=json.load(sys.stdin); assert 'reviewed_candidate' in d"` | 0 |
| 5 | Nonzero denominator | stdout of check 4 contains `"checked":` with value > 0 — assert with `jq '.reviewed_candidate.checked > 0'` | 0 |
| 6 | No source file exceeds 80 lines | `find crates/review -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +` | 0 |
| 7 | Semgrep binary available and emits JSON | `semgrep scan --json --config auto crates/review/src/ \| python3 -c "import json,sys; json.load(sys.stdin)"` | 0 |
| 8 | Ruff binary available and emits JSON | `ruff check --output-format json crates/review/src/ ; echo "exit: $?"` — non-zero is allowed when findings exist; malformed JSON is the failure | JSON-parseable stdout |
| 9 | Mutation floor | Manual: flip each row in §11; confirm the named test goes red. `caught/total >= 80%` | — |
| 10 | Independent-model review recorded | Receipt contains `reviewer_id != worker_id` and non-empty `input_digest` + `output_digest` | — |

## 14. Failure stories and review questions
**Failure → Cause → Fix:** reviewer saw a different tree than verifier → review lacked frozen digest → verify exact digest equality before forwarding.
1. Who reviews the reviewer? 2. Is zero-file scan failure? 3. Can a model accept its own patch?
