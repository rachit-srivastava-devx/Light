# L8 Blocker Closeout — 2026-09-12

Scope: adversarial spot-check of three blockers declared fixed, plus the non-blocking intent floor.
Method: direct read of `verify/`, `rollback/`, `control/`, `intent/`, `approval/` BLUEPRINT.md + corroborating greps.

## Blocker 1 (verify §9): STILL OPEN

The **format** half is fixed; the **semantic** half is not.

Path prefixes — all 5 steps now lead with a file. Step 2 and step 4 verbatim:

> 2. In `src/runner.rs`: Implement `run_gate` using `tokio::process::Command` to invoke the gate binary; port denominator logic from `src/pipeline/verify_stage.rs:23-44`; `cargo test -p verify verify::tests::reviewed_candidate_runs_deterministic_gates` exits 0.

> 4. In `src/secret.rs`: Implement Gitleaks invocation via `Command::new("gitleaks")` with `--no-banner --redact --json` flags and redacted-finding normalization into `SecretResult`; `cargo test -p verify verify::tests::coverage_below_floor_marks_gate_failed` exits 0.

Two defects remain:

1. **Step 4 pairs the wrong test with the file it edits.** The step edits `src/secret.rs` (Gitleaks invocation, `SecretResult` normalization) but gates on `coverage_below_floor_marks_gate_failed` — a coverage test. §11 assigns coverage to `evaluate_coverage` in `src/runner.rs`, a different file. A builder can implement `src/secret.rs` as `todo!()` and still turn that step green, and can break secret scanning entirely without turning it red. The step's done-condition is neither necessary nor sufficient for the step's work. This is the exact class of defect Blocker 1 was raised against — prose that does not bind to an executable check — and it survives in a step that now merely *looks* compliant.

2. **No secret-scan test exists anywhere to bind it to.** All three named tests in §10 are gate/evidence/coverage tests. `normalize_findings` in §11 correspondingly has no real catcher (see Blocker 3). Step 4 cannot be fixed by swapping the test name — a fourth named test has to be added to §10 first.

Minor / literal: step 1 ends with `cargo check -p verify`, not `cargo test -p verify <test_name>`. For a pure type-definition step this is defensible and I do not hold it against the blocker, but it does not meet the stated criterion as written.

## Blocker 2 (file-path consistency): STILL OPEN

2 of 3 spot-checked nodes clean; **intent is not fixed**.

**control — PASS (literal).** §4 declares `lib.rs`, `reducer.rs`, `scheduler.rs`, `supervisor.rs`, `tests/recovery.rs`. All 5 §11 "Function mutated" cells cite `src/reducer.rs`, which is declared.

> | `emit_intent` in `src/reducer.rs` | Suppress intent emission — return `Ok` without writing an `IntentSpec` to the outbox | `control::tests::ingest_event_drives_state_then_intent` | test asserts exactly one intent is emitted after a valid event; suppressing emission leaves the outbox empty and the assertion fails |

Flag (not the blocker, but adjacent): §11 rows 4 and 5 attribute `Move spawn before store commit` and `Accept a stale child generation` to `reduce` in `src/reducer.rs`. §5 states "`reduce` is pure" and §9 step 4 places spawn/`AuthorityStore::transact` in `src/supervisor.rs`. A pure reducer cannot host a spawn-ordering or child-generation mutation. The filename is declared, so it passes the literal check while still pointing at the wrong file.

**approval — PASS (literal).** §4 declares `lib.rs`, `grant.rs`, `check.rs`, `port.rs`, `tests/approval.rs`. All 5 §11 cells cite `src/check.rs`, declared.

> | `consume` in `src/check.rs` | Change consume to a read-only fetch — do not mark the grant consumed | `approval::tests::replay_refused_after_consume` | test calls consume twice with the same id; a read-only consume lets the second call return `Ok` and the `Err(Replay)` assertion fails |

Flag: §9 step 4 places the durable `ApprovalStore` implementation (which owns `consume`) in `src/port.rs`, while §11 mutates `consume` in `src/check.rs`. Same wrong-file pattern as control.

**intent — FAIL.** §4 declares `lib.rs`, `schema.rs`, `prompt.rs`, `gate.rs`, `tests/adversarial.rs`. §11's four cells are all clean and declared:

> | `independent_kind` in `src/gate.rs` | Remove conservative path; always use the model's proposed kind | `intent::tests::disagreement_chooses_conservative` | Test checks that model/deterministic disagreement routes the conservative kind; removing the path returns the model choice, breaking the kind assertion |

But **§10 cites `src/parse.rs` twice — a file that does not exist in §4** (`intent/BLUEPRINT.md:114` and `:116`):

- line 114: "Catches removal of the kind allow-list guard in `src/parse.rs`"
- line 116: "Catches removal of the extra-field rejection in `src/parse.rs`"

§11 places both of those exact guards in `src/schema.rs` (rows 1 and 4). So §10 and §11 now disagree with each other about where the same two guards live, and §10's answer names a phantom file. The fix moved the mismatch out of §11 rather than removing it. A builder reading §10 creates `src/parse.rs`; the §12 per-file line-count gate and the §11 mutation targets both assume `src/schema.rs`.

## Blocker 3 (Function mutated format): STILL OPEN

**Function mutated column — fixed in both.** Every row in verify §11 and rollback §11 carries a backtick-quoted snake_case identifier (`run_gate`, `normalize_findings`, `assemble_gate_evidence`, `evaluate_coverage`; `check_containment`, `check_cas`, `check_already_rolled_back`, `apply_shared_ref_action`, `write_receipt`, `verify_action`). Mutation column carries the mutation. No descriptor phrases in column 1.

**Test column — not fixed in verify.** Rows 3 and 4 hold `verify::tests::...` — a literal ellipsis placeholder, not a qualified test name:

verify §11, first 3 rows:

> | `run_gate` in `src/runner.rs` | Skip one gate | `verify::tests::reviewed_candidate_runs_deterministic_gates` | Denominator/hidden test: all inputs checked; omitting a gate reduces checked/total and fails the assertion |
> | `run_gate` in `src/runner.rs` | Return pass for empty input set | `verify::tests::reviewed_candidate_runs_deterministic_gates` | Zero-input test: no empty success; an empty gate input must fail, not pass |
> | `normalize_findings` in `src/secret.rs` | Ignore secret finding in output | `verify::tests::...` (security variant) | Secret fixture: security finding blocks; redacted findings must be present in the result |

Row 4 has the same defect: ``| `assemble_gate_evidence` in `src/receipt.rs` | ... | `verify::tests::...` (differential variant) | ...``

`verify::tests::...` is not a runnable test. Two of verify's eight mutation targets therefore have no named catcher, and the §13 mutation floor (`caught/total >= 80 %`) cannot be evaluated against them. This is the same "test names in the wrong column / not real names" failure Blocker 3 named, relocated rather than closed.

**rollback §11 — clean.** All 6 rows carry fully qualified test names. First 3 rows:

> | `check_containment` in `src/guard.rs` | Remove path-containment predicate, accepting any `PathBuf` target | `rollback::tests::path_escape_is_refused` | Rollback cannot delete user data outside owned root; removing the check is the exact attack |
> | `check_cas` in `src/guard.rs` | Skip current-HEAD comparison, always treating HEAD as matching applied commit | `rollback::tests::private_rollback_emits_receipt` with a post-apply HEAD change | No clobber of newer work; a moved HEAD must refuse |
> | `check_already_rolled_back` in `src/guard.rs` | Remove terminal-status guard, allowing a second rollback attempt | `rollback::tests::nonexistent_artifact_is_refused` (second-call variant) | Action is one-use; a receipt with terminal status must refuse replay |

Same ellipsis defect also survives outside the two files under review — `dag/BLUEPRINT.md:98`, `notify/BLUEPRINT.md:125`, `notify/BLUEPRINT.md:126`. The fix pass was applied per-file, not corpus-wide.

## intent floor: CLOSED

All three sections agree on 75%. No 80% remains in the file.

- §11 (`intent/BLUEPRINT.md:129`): "Safety mutation floor: `caught/total >= 75%`."
- §12 (`:139`): "`# Mutation floor: caught/total >= 75%`"
- §13 (`:153`): "Mutation floor: `caught/total >= 75%` for all authority predicates in §11; manually deleting the extra-field rejection causes `intent::tests::extra_authority_field_refuses` to fail."

## OVERALL: FAIL

Only the non-blocking intent floor is genuinely closed; all three blockers survive in reduced but real form — verify §9 step 4 gates a secret-scan file on a coverage test, intent §10 still names the phantom `src/parse.rs` that §11 calls `src/schema.rs`, and verify §11 still carries two `verify::tests::...` placeholders where a runnable test name is required.

### Secondary findings (not blockers, raised for the next pass)

- `control/BLUEPRINT.md:157` — §13 names "the 3 named mutation targets in §11 (launch-before-commit, stale-generation, infinite-retry)" but §11 lists 5 rows and contains no infinite-retry target.
- `approval/BLUEPRINT.md:137` — §13 names "the 3 named mutation targets in §11" against a §11 with 5 rows.
- `control/BLUEPRINT.md:127-128` — §11 test column holds descriptor phrases ("kill-after-dispatch recovery test", "late-child hidden test"), the same Blocker-3 defect class, in a file not covered by this review's Blocker-3 scope.
- `rollback/BLUEPRINT.md:101` vs `:141` — §10 says "these 3 must compile and pass" but §13 requires a fourth, `rollback::tests::rollback_emits_requeue_edge_on_shared_ref`, which §9 step 5 and §11 row 4 also depend on.
