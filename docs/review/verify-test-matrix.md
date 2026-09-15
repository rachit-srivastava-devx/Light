# Node 2 verify test matrix

This is a checked-in test contract for the verify node. It is intentionally separate from
production implementation and authored acceptance suites.

| Layer | Fixture | Proof |
|---|---|---|
| Unit | `crates/verify/src/matrix_tests.rs` | digest binding, integer denominators, zero-input refusal, redaction, canonical runner, mutation sensitivity |
| Golden | `crates/verify/tests/golden_dataset/` | deterministic serialized input and exact expected legacy evidence digest/status |
| Integration | existing `crates/verify/tests/gitleaks_real/` | installed scanner, real JSON, redacted finding |
| E2E | `src/tests/verify_real_binary_matrix/` | compiled `fleet` binary, real temporary repositories, pass/fail denominator and secret redaction |

Mutation-style coverage uses a local deliberately weakened implementation and asserts that the
matrix distinguishes it from the real zero-input behavior. It does not modify authored acceptance
tests or production code. A complete Node 2 claim still requires the real commands and Fleet review
evidence to be recorded in `docs/LLD_PROGRESS.md`.
