# BLUEPRINT — `verify`

## 1. Identity and LLD path
- Node id / label / tag: `verify / Verify + Secret Scan / deterministic`
- LLD authority: `docs/LLD/LLD.md §13, §18` and `docs/LLD/lld-full-detail.architecture.json:components[id=verify]`
- Why this node exists: run protected real tests and two-tier secret/policy evidence on the exact reviewed tree.
- Incoming edges: `review -> verify` (`reviewed candidate`).
- Outgoing edges: `verify -> integrate` (`pass`), `verify -> candidate` (`evidence`).
- Build status: `partial` — `crates/fleet-verify` and `src/pipeline/verify_stage.rs:23-44` are existing seams, not the complete LLD node.

## 2. Responsibility and non-goals
**Owns:** gate enumeration/execution, secret scan, result normalization, denominators, evidence digest, and typed pass/fail/refusal.
**Does not own:** model review, source mutation, merge, rollback, or publication.

## 3. Boundary and authority
Deterministic authority over verification evidence. It runs named gates with real binaries, protects acceptance inputs, and records every input/output. A gate examining zero inputs fails. `0` is pass, `3` environment fault, `6` invariant, `7` refusal, `8` mismatch. A model cannot turn failure into pass.

## 4. Crate/package layout
```text
crates/verify/
  Cargo.toml
  src/lib.rs             # ≤40 lines
  src/runner.rs          # ≤80 lines
  src/secret.rs          # ≤80 lines
  src/receipt.rs         # ≤80 lines
  tests/real_binary.rs   # ≤80 lines
```

> **Crate status:** `verify` is a NEW crate at `crates/verify/`. Do not confuse with the existing `crates/fleet-verify/` — that crate is a separate existing module. The §7 references to fleet-verify are extraction/reuse candidates, not the build target.

## 5. Public API contract
```rust
pub trait GateRunner: Send + Sync { fn run(&self, gate: &GateSpec, tree: &Tree) -> Result<GateResult, VerifyError>; }
pub fn verify(input: &VerifyInput, runner: &dyn GateRunner) -> Result<VerifyReport, VerifyError>;
pub struct VerifyInput { pub tree_digest: String, pub acceptance_digest: String, pub gates: Vec<GateSpec> }
pub struct VerifyReport { pub results: Vec<GateResult>, pub checked: u64, pub total: u64, pub evidence_digest: String, pub status: Status }
```
Precondition: gates and tree are nonempty and immutable. Postcondition: every named gate has a result, `checked == total > 0`, and pass requires all pass. No fake child is the only real-binary proof.

## 6. Data model, invariants, and failure taxonomy
| Type / record | Invariant | Illegal state prevented | Failure / exit code |
|---|---|---|---|
| `GateSpec` | command, input set, protected flag | unnamed check | `InvalidGate` / 7 |
| `GateResult` | exit/stdout/stderr/input digest recorded | opaque green | `Mismatch` / 8 |
| report | every gate enumerated; positive denominator | skipped inputs | `ZeroCoverage` / 6 |
| secret result | files/commits scanned and findings redacted | false clean | `SecretFound` / 6 |

Clock, subprocess, filesystem, scanner and environment are injected/isolated. Use integers/fixed strings; no floats or `unwrap_or(0)`.

## 7. Reuse map and adoption decisions
| Existing source or stable library | Evidence/version/commit/license | Use | Why not custom code | Proof still required |
|---|---|---|---|---|
| `src/pipeline/verify_stage.rs:23-44` | exact graph source | report/real runner composition seam | preserve existing gate dispatch | complete denominator/secret contract |
| `crates/fleet-verify` tests `report_semantics.rs:40-56` | graph search result | count semantics | retain known zero-count behavior | node contract |
| Cargo/rustc 1.98.0; MIT/Apache-2.0 | quality ledger | native tests/real binary | workspace-native | actual environment output |
| Gitleaks 8.30.1, MIT; documented/local failure observed | quality ledger | secret scan | maintained scanner | triage findings, no clean claim |
| cargo-mutants 27.1.0, MIT; documented only | quality ledger | mutation evidence | established mutation tool | local run and nonzero mutants |
| `serde = { version = "1", features = ["derive"] }` | crates.io 1.0.x; MIT/Apache-2.0 | derive `Deserialize`/`Serialize` for config + evidence types | workspace-standard JSON serde | `cargo test` passes with real derive |
| `serde_json = "1"` | crates.io 1.0.x; MIT/Apache-2.0 | parse `cargo llvm-cov --json --summary-only` output | workspace-standard | coverage JSON deserialized without panics |
| `thiserror = "2"` | crates.io 2.x; MIT/Apache-2.0 | typed `VerifyError` variants with `#[from]` | eliminates hand-written `Display`/`From` | error variants reachable via `?` in tests |
| `tokio = { version = "1", features = ["rt-multi-thread", "process", "macros"] }` | crates.io 1.x; MIT | `tokio::process::Command` for async subprocess; `#[tokio::test]` | fleet-wide async runtime; required by `process` feature | integration tests run under `#[tokio::test]` |
| `cargo-llvm-cov` (external binary, NOT a crate dep) | cargo-llvm-cov 0.6.x; MIT | coverage measurement | llvm-native; no custom instrumentation | `cargo llvm-cov --json --summary-only` exits 0; JSON parsed |

### Cargo.toml snippet (pinned)
```toml
[dependencies]
serde      = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror  = "2"
tokio      = { version = "1", features = ["rt-multi-thread", "process", "macros"] }

[dev-dependencies]
# cargo-llvm-cov is installed as a binary: `cargo install cargo-llvm-cov`
# Invoked via subprocess in tests or CI — not listed here.
```

### `cargo-llvm-cov` invocation and JSON parsing
```bash
# Produces a single-object JSON with integer `data[0].totals.lines.count` and
# `data[0].totals.lines.covered` fields.
cargo llvm-cov --json --summary-only 2>/dev/null
```
Parse with `serde_json`:
```rust
#[derive(serde::Deserialize)]
struct LlvmCovSummary { data: Vec<LlvmCovData> }
#[derive(serde::Deserialize)]
struct LlvmCovData { totals: LlvmCovTotals }
#[derive(serde::Deserialize)]
struct LlvmCovTotals { lines: LlvmCovLines }
#[derive(serde::Deserialize)]
struct LlvmCovLines { count: u64, covered: u64 }

fn parse_coverage_counts(json: &str) -> Result<(u64, u64), VerifyError> {
    let s: LlvmCovSummary = serde_json::from_str(json)
        .map_err(|e| VerifyError::CoverageParseError(e.to_string()))?;
    s.data.into_iter().next()
        .map(|d| (d.totals.lines.covered, d.totals.lines.count))
        .ok_or(VerifyError::CoverageParseError("empty data array".into()))
}
```
Compare coverage using integer basis points, for example
`covered.checked_mul(10_000) / count >= floor_bps`; never deserialize or compute a
floating-point percentage.

## 8. Behavior matrix
### `verify`
| Input/failure dimension | Observable behavior |
|---|---|
| empty / missing | refusal and receipt; `0/0` never pass |
| wrong type / Unicode | typed gate/config error |
| huge / negative | bounded output and checked counts; invalid limits refuse |
| duplicate / concurrent | duplicate gate IDs conflict; immutable tree prevents races |
| partial failure / timeout | record failure/timeout; stop or continue per gate policy; never pass incomplete |
| stale / unavailable | tree/acceptance digest mismatch fails; missing tool is exit 3 |

## 9. Tiny implementation steps

1. In `src/lib.rs`: Define `GateSpec`, `GateResult`, `VerifyInput`, `VerifyReport`, `VerifyError`, and `Status` types; re-export all public items; `cargo check -p verify` exits 0.
2. In `src/runner.rs`: Implement `run_gate` using `tokio::process::Command` to invoke the gate binary; port denominator logic from `src/pipeline/verify_stage.rs:23-44`; `cargo test -p verify verify::tests::reviewed_candidate_runs_deterministic_gates` exits 0.
3. In `src/receipt.rs`: Implement `assemble_gate_evidence` with per-gate `input_digest`, `output_digest`, and positive `checked/total` denominator; `cargo test -p verify verify::tests::failing_gate_produces_gate_evidence_not_panic` exits 0.
4. In `src/secret.rs`: Implement Gitleaks invocation via `Command::new("gitleaks")` with `--no-banner --redact --json` flags and redacted-finding normalization into `SecretResult`; `cargo test -p verify verify::tests::secret_finding_blocks_gate` exits 0.
5. In `tests/real_binary.rs`: Wire all three named integration tests end-to-end using fixture repo at `tests/fixtures/blueprint-verify/`; `cargo test -p verify reviewed_candidate_runs_deterministic_gates failing_gate_produces_gate_evidence_not_panic coverage_below_floor_marks_gate_failed` all pass.

## 10. Test matrix
**Unit:** gate enumeration, zero input, failure mapping, redaction, digest mismatch.
**Integration/contract:** review digest → verify real candidate → integrate pass/refusal.
**Hidden:** fake child, skipped gate, changed acceptance, scanner zero files, tool missing.
**Property:** report pass iff all generated gates pass and `checked==total>0`; fixed seed 300 cases.
**Differential:** canonical reference/candidate outcomes over immutable corpus; compare exit, receipts, state, hashes, not only zero exit.
**Real-binary:** `env!("CARGO_BIN_EXE_fleet")` plus `cargo test --workspace --no-fail-fast`; fake child is supplemental only.

### Named integration tests (these 5 must compile and pass)

| Test name | Inputs | Expected output | Mutation caught |
|---|---|---|---|
| `verify::tests::reviewed_candidate_runs_deterministic_gates` | `ReviewedCandidate` with a known repo fixture (pre-seeded tree digest + at least one `GateSpec`); no environment fault | `GateEvidence` produced; `gate_results` has `len() >= 1`; `checked > 0` and `checked == total`; `status` is not `Refused` | LLD edge contract: `run_gate` → short-circuit pass (skips enumeration) would produce `checked == 0` and fail this assertion |
| `verify::tests::failing_gate_produces_gate_evidence_not_panic` | `ReviewedCandidate` with a `GateSpec` whose command exits non-zero against the fixture | `GateEvidence` returned (not panic/unwrap); `passed == false`; `failures` list is non-empty | Mutating `run_gate` to always return `GateResult::Pass` yields `passed == true` and `failures.is_empty()`, which fails `assert!(!evidence.passed)` |
| `verify::tests::coverage_below_floor_marks_gate_failed` | `ReviewedCandidate` with coverage floor set to `80`; fixture repo has `70 %` line coverage as reported by `cargo llvm-cov --json --summary-only` stub | Gate for coverage reports `passed == false`; failure message contains the observed percent and the floor | Mutating `evaluate_coverage` to always return `100.0` yields `passed == true`, failing `assert!(!gate.passed)` |
| `verify::tests::secret_finding_blocks_gate` | `ReviewedCandidate` with a fixture tree containing a secret in a file; `FindingsProvider` mock returns one finding with `severity = "CRITICAL"` | `GateEvidence` returned; `passed == false`; secret finding present in `findings` list | Mutating `normalize_findings` to return empty findings yields `passed == true`, failing `assert!(!evidence.passed)` |
| `verify::tests::evidence_digest_mismatch_refused` | `ReviewedCandidate` with all gate results passing but `output_digest` set to a value that does not match the recomputed digest from gate outputs | `GateEvidence` returned; `passed == false`; error message references digest mismatch | Mutating `assemble_gate_evidence` to skip the digest comparison yields `passed == true`, failing `assert!(!evidence.passed)` |

**Anti-stub guarantee:** each test is only satisfiable by running the real gate path.
`reviewed_candidate_runs_deterministic_gates` requires a real `GateEvidence` struct (not a default), so a stub returning `Default::default()` fails on `checked > 0`.
`coverage_below_floor_marks_gate_failed` requires real JSON parsing; a stub returning `passed: true` fails on `assert!(!gate.passed)`.

## 11. Mutation targets and anti-stub proof
| Function mutated | Mutation applied | Test that catches it | Why it's the right catcher |
|---|---|---|---|
| `run_gate` in `src/runner.rs` | Skip one gate | `verify::tests::reviewed_candidate_runs_deterministic_gates` | Denominator/hidden test: all inputs checked; omitting a gate reduces checked/total and fails the assertion |
| `run_gate` in `src/runner.rs` | Return pass for empty input set | `verify::tests::reviewed_candidate_runs_deterministic_gates` | Zero-input test: no empty success; an empty gate input must fail, not pass |
| `normalize_findings` in `src/secret.rs` | Ignore secret finding in output | `verify::tests::secret_finding_blocks_gate` | Secret fixture: security finding blocks; redacted findings must be present in the result |
| `assemble_gate_evidence` in `src/receipt.rs` | Compare only exit code, drop digests | `verify::tests::evidence_digest_mismatch_refused` | Differential fixture: receipts/state/digests matter; all evidence fields must be present |
| `run_gate` in `src/runner.rs` | Replace real binary with fake | `verify::tests::reviewed_candidate_runs_deterministic_gates` | Real-binary test: product path is exercised; the real gate binary must run |
| `run_gate` in `src/runner.rs` | Always return `GateResult::Pass` | `verify::tests::failing_gate_produces_gate_evidence_not_panic` | `assert!(!evidence.passed)` fails when mutant passes every gate unconditionally |
| `evaluate_coverage` in `src/runner.rs` | Always return `coverage = 100.0` | `verify::tests::coverage_below_floor_marks_gate_failed` | `assert!(!gate.passed)` fails when mutant reports coverage above the 80 % floor |
| `assemble_gate_evidence` in `src/receipt.rs` | Return empty `failures` list | `verify::tests::failing_gate_produces_gate_evidence_not_panic` | `assert!(!evidence.failures.is_empty())` fails when mutant discards the failure entries |

**Named functions to mutate (cargo-mutants targets these explicitly):**
- `run_gate` in `src/runner.rs` — authority predicate; floor 80 %.
- `evaluate_coverage` in `src/runner.rs` — coverage threshold logic; floor 80 % from first pass.
- `assemble_gate_evidence` in `src/receipt.rs` — evidence assembly; floor 80 % from first pass.

Mutation target floor: `caught/total >= 80 %`. Manual reviewer kills skipped-input mutant if `cargo-mutants` does not surface it automatically.

## 12. Verification recipe and denominators
```bash
cargo test --workspace --no-fail-fast
cargo clippy --workspace --all-targets -- -D warnings
find crates/verify -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +
target/debug/fleet verify --id verify-blueprint-smoke
gitleaks git --no-banner --redact
cargo mutants --workspace --timeout 60
```
Expected evidence must include real exit/output, gate `checked=n,total=n,n>0`, mutation `discovered>0,killed/total`, scanner files/commits, and exact tree/acceptance digests. Current local research records Gitleaks failure and leaves mutation/remote/production proof unverified.

## 13. Definition of done

Each item is a checkable assertion, not a qualitative statement. All must be green before marking the slice done.

```
[ ] cargo test -p verify exits 0
    Denominator: all 5 named integration tests present and passing
    (verify::tests::reviewed_candidate_runs_deterministic_gates,
     verify::tests::failing_gate_produces_gate_evidence_not_panic,
     verify::tests::coverage_below_floor_marks_gate_failed,
     verify::tests::secret_finding_blocks_gate,
     verify::tests::evidence_digest_mismatch_refused)

[ ] `verify::tests::evidence_digest_mismatch_refused` appears in test output and passes.

[ ] cargo clippy -p verify --all-targets -- -D warnings exits 0
    Denominator: zero warnings; no allow attributes hiding real issues

[ ] find crates/verify -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} + exits 0
    Denominator: every .rs file in crates/verify/ (including tests/) is ≤80 lines; awk tracks per-file counts; exits 1 on first violation

[ ] cargo llvm-cov -p verify --json --summary-only | jq '.data[0].totals.lines.percent >= 80'
    Denominator: line coverage for the verify crate is ≥80 %

[ ] cargo mutants -p verify --timeout 60 exits with caught/total >= 0.80
    Denominator: run_gate, evaluate_coverage, and assemble_gate_evidence each have ≥1 killing test

[ ] gitleaks git --no-banner --redact exits 0 on the verify crate tree
    Denominator: files/commits scanned > 0; no unredacted finding

[ ] target/debug/fleet verify --id verify-blueprint-smoke exits 0
    Denominator: GateEvidence emitted to stdout with checked=n, total=n, n>0

[ ] All gate results include exit code, stdout digest, stderr digest, and input-set digest
    Denominator: GateEvidence struct fields non-empty for each GateResult

[ ] A missing required tool (e.g. gitleaks not on PATH) exits 3, not 0 or panic
    Denominator: integration test with tool removed from PATH asserts exit code 3
```

## 14. Failure stories and review questions
**Failure → Cause → Fix:** a gate read zero inputs and passed → runner treated empty output as success → enumerate input set and fail `checked=0`.
1. Did the real binary run? 2. What exact files/commits were scanned? 3. Does a missing tool become exit 3 rather than a false agent failure?
