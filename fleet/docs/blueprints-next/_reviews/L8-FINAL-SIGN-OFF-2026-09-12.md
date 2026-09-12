# L8 Final Sign-Off — 2026-09-12

Scope: confirm the two orphan test names are wired into both §10 (named integration
test matrix) and §13 (definition of done) of their blueprints, and that neither file
carries a `tests::...` ellipsis placeholder. No new-issue hunt was performed.

Files under review (absolute):
- `/Users/rachitsrivastava/youtube/Principal Engineering/Light/fleet/docs/blueprints-next/verify/BLUEPRINT.md`
- `/Users/rachitsrivastava/youtube/Principal Engineering/Light/fleet/docs/blueprints-next/notify/BLUEPRINT.md`

## verify orphan fix: CLOSED

**§10 — 5 named rows present** (`verify/BLUEPRINT.md:129` header reads "Named integration
tests (these 5 must compile and pass)"; rows at lines 133–137):

| # | Test name | Line |
|---|---|---|
| 1 | `verify::tests::reviewed_candidate_runs_deterministic_gates` | 133 |
| 2 | `verify::tests::failing_gate_produces_gate_evidence_not_panic` | 134 |
| 3 | `verify::tests::coverage_below_floor_marks_gate_failed` | 135 |
| 4 | `verify::tests::secret_finding_blocks_gate` | 136 |
| 5 | `verify::tests::evidence_digest_mismatch_refused` | 137 |

Quoted row 5 (`verify/BLUEPRINT.md:137`):

> `| `verify::tests::evidence_digest_mismatch_refused` | `ReviewedCandidate` with all gate results passing but `output_digest` set to a value that does not match the recomputed digest from gate outputs | `GateEvidence` returned; `passed == false`; error message references digest mismatch | Mutating `assemble_gate_evidence` to skip the digest comparison yields `passed == true`, failing `assert!(!evidence.passed)` |`

**§13 DoD — bullet present.** Quoted (`verify/BLUEPRINT.md:186`):

> ``[ ] `verify::tests::evidence_digest_mismatch_refused` appears in test output and passes.``

It also appears inside the §13 denominator enumeration for `cargo test -p verify`
(`verify/BLUEPRINT.md:184`), so the DoD's "all 5 named integration tests" denominator
is closed over the same set as §10 — no drift between the two sections.

**§11 `normalize_findings` row — intact.** Quoted (`verify/BLUEPRINT.md:148`):

> `| `normalize_findings` in `src/secret.rs` | Ignore secret finding in output | `verify::tests::secret_finding_blocks_gate` | Secret fixture: security finding blocks; redacted findings must be present in the result |`

File, mutation, and catcher are unchanged: `src/secret.rs` /
`verify::tests::secret_finding_blocks_gate`. The new §10 row did not displace it.

## notify orphan fix: CLOSED

**§10 — 4 named rows present** (`notify/BLUEPRINT.md:103` header reads "Named integration
tests (these 4 must compile and pass)"; rows at lines 107–110):

| # | Test name | Line |
|---|---|---|
| 1 | `notify::tests::state_change_emits_redacted_notification` | 107 |
| 2 | `notify::tests::sensitive_fields_not_in_notification` | 108 |
| 3 | `notify::tests::duplicate_delivery_refused` | 109 |
| 4 | `notify::tests::zero_denominator_vacuous_proof_refused` | 110 |

Quoted row 4 (`notify/BLUEPRINT.md:110`):

> `| `notify::tests::zero_denominator_vacuous_proof_refused` | `NotifyInput` where `delivery_attempts = 0` and `total_recipients = 0`; invoke `build_delivery_receipt` | `Err(NotifyError::ZeroDenominator)` returned; no receipt written | Mutating `build_delivery_receipt` to return `checked=0, total=0` vacuously yields a pass, failing `assert!(result.is_err())` |`

**§13 DoD — bullet present.** Quoted (`notify/BLUEPRINT.md:147`):

> ``- `notify::tests::zero_denominator_vacuous_proof_refused` appears in test output and passes.``

The test also has a §11 mutation-target row backing it (`notify/BLUEPRINT.md:127`,
`build_delivery_receipt` in `src/reconcile.rs`), so the name is anchored in all three
sections, not just declared.

## Ellipsis-free: CONFIRMED

Command run in `docs/blueprints-next/`:

```
grep -n 'tests::\.\.\.' verify/BLUEPRINT.md notify/BLUEPRINT.md
```

Exit 1, zero matches — no `tests::...` placeholder in either file.

A wider `grep -n '\.\.\.'` over both files returns exactly one hit,
`notify/BLUEPRINT.md:152`:

> ``- `Cargo.toml` pins `serde = { version = "1", ... }`, `thiserror = "2"`, `tokio = { version = "1", ... }` exactly as shown in §7.``

That is TOML field elision inside a dependency-pin assertion, not a test-name
placeholder. Both files are clean of the defect class under review.

## OVERALL VERDICT: PASS

All five checks pass: both orphan tests are now named in §10 and asserted in §13 of
their blueprints, `verify` §11's `normalize_findings` → `secret_finding_blocks_gate`
mapping survived the edit, and neither file contains a `tests::...` placeholder.

## What a builder can now do

Any agent handed `verify/BLUEPRINT.md` or `notify/BLUEPRINT.md` can read a closed set
of named tests off §10, implement against §5's signatures and §9's per-file steps, and
self-check with §13's commands without asking what "done" means — every test named in
the DoD has a matching §10 row specifying inputs, expected output, and the mutant it
kills, and every §11 mutation target names a real catcher. The blueprints are executable
briefs: the acceptance set is fixed before the first line of code, so a builder cannot
negotiate the denominator down mid-build.

---
Scope note (not part of the verdict, not a blocker): while reading the exact sections
under check, the `notify` §13 bullet list names `failed_delivery_does_not_panic`
(`notify/BLUEPRINT.md:146`) but not `duplicate_delivery_refused`, while §10 names the
reverse. Outside the five checks I was asked to assess; recorded here for whoever owns
the next pass rather than actioned.
