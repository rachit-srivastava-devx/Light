# L8 Blocker Closeout v2 — 2026-09-12

Scope: re-check of the three blockers raised in `L8-BLOCKER-CLOSEOUT-2026-09-12.md` after fixes,
plus the two secondary file-mapping issues. Evidence is quoted from the files as they stand.

---

## Blocker 1 (verify §9/§10 alignment): CLOSED

**§9 step 4** — `verify/BLUEPRINT.md:118`. Now names `src/secret.rs` and gates on the secret test,
not the coverage test:

> 4. In `src/secret.rs`: Implement Gitleaks invocation via `Command::new("gitleaks")` with `--no-banner --redact --json` flags and redacted-finding normalization into `SecretResult`; `cargo test -p verify verify::tests::secret_finding_blocks_gate` exits 0.

File and gate now agree: the step that writes `src/secret.rs` is gated by the test that exercises
`src/secret.rs`. The previous mismatch (secret-scan step gated on `coverage_below_floor_marks_gate_failed`)
is gone.

**§10 Named integration tests** — 4th row present at `verify/BLUEPRINT.md:136`:

> | `verify::tests::secret_finding_blocks_gate` | `ReviewedCandidate` with a fixture tree containing a secret in a file; `FindingsProvider` mock returns one finding with `severity = "CRITICAL"` | `GateEvidence` returned; `passed == false`; secret finding present in `findings` list | Mutating `normalize_findings` to return empty findings yields `passed == true`, failing `assert!(!evidence.passed)` |

The row carries real inputs, a real assertion (`assert!(!evidence.passed)`), and a named mutation —
not a placeholder.

**§11 `normalize_findings` row** — `verify/BLUEPRINT.md:147`:

> | `normalize_findings` in `src/secret.rs` | Ignore secret finding in output | `verify::tests::secret_finding_blocks_gate` | Secret fixture: security finding blocks; redacted findings must be present in the result |

Catcher column holds a qualified, runnable test name. The `verify::tests::...` ellipsis is gone.

**Residual (minor, new):** §9 step 5 (`verify/BLUEPRINT.md:119`) still reads "Wire all **three** named
integration tests" while §10 now lists four. The three it enumerates are the real-binary ones and
`secret_finding_blocks_gate` is gated in step 4, so nothing is unbuildable — but the count is stale.

---

## Blocker 2 (intent §10 filename): CLOSED

`grep -n "parse\.rs" intent/BLUEPRINT.md` → **no matches**. The phantom `src/parse.rs` is fully gone
from `intent`. (It survives only in `user_cli/BLUEPRINT.md`, where `src/parse.rs` is legitimately
declared in that crate's own §4 — correct, not a leak.)

**§4 layout** — `intent/BLUEPRINT.md:25-33` declares `src/schema.rs`:

```text
crates/intent/
  Cargo.toml
  src/lib.rs                 # types/ports, 40 lines
  src/schema.rs              # IntentSpec validation, 80 lines
  src/prompt.rs              # versioned prompt/context projection, 70 lines
  src/gate.rs                # deterministic independent check, 80 lines
  tests/adversarial.rs       # hidden prompt-injection and malformed output tests, 80 lines
```

**§10 rows now name `src/schema.rs`** — `intent/BLUEPRINT.md:114` and `:116`:

> | `intent::tests::unknown_kind_refuses` | … | `Err(IntentError::UnknownKind)` | Catches removal of the kind allow-list guard in **`src/schema.rs`**; accepting unknown kinds lets arbitrary work reach route without schema validation |

> | `intent::tests::extra_authority_field_refuses` | … | `Err(IntentError::AuthorityField)` | Catches removal of the extra-field rejection in **`src/schema.rs`**; allowing authority fields enables forgery; any stub that strips and accepts unknown fields passes the field check silently |

§4, §9 step 2, §10 and §11 now all name the same file. Fully consistent.

---

## Blocker 3 (ellipsis-free §11): CLOSED

`grep -rn "tests::\.\.\." --include=BLUEPRINT.md .` over the whole tree → **zero matches in any
BLUEPRINT.md**. The only hits are inside the prior review doc itself, quoting the old defect.
A broader sweep for bare-`...` table cells also returns nothing.

**verify** — both former ellipsis rows (`verify/BLUEPRINT.md:147`, `:148`):

> | `normalize_findings` in `src/secret.rs` | Ignore secret finding in output | `verify::tests::secret_finding_blocks_gate` | Secret fixture: security finding blocks; redacted findings must be present in the result |

> | `assemble_gate_evidence` in `src/receipt.rs` | Compare only exit code, drop digests | `verify::tests::evidence_digest_mismatch_refused` | Differential fixture: receipts/state/digests matter; all evidence fields must be present |

**notify** — all five §11 rows carry qualified names or an explicit non-test descriptor; the former
placeholder rows now read (`notify/BLUEPRINT.md:124`, `:126`):

> | `handle_delivery_failure` in `src/reconcile.rs` | Treat transport error as success (`DeliveryStatus::Delivered`) or call `unwrap()` | `notify::tests::failed_delivery_does_not_panic` | Transport failure must produce `Unknown`/logged error; not panic and not fabricated success |

> | `build_delivery_receipt` in `src/reconcile.rs` | Return `checked=0, total=0` vacuously | `notify::tests::zero_denominator_vacuous_proof_refused` | No vacuous delivery proof; nonzero denominator required |

**dag** — all five rows named; the former placeholders now read (`dag/BLUEPRINT.md`):

> | `validate` in `src/graph.rs` | Accept a zero-node `GraphVersion` as `Ok(())` | `dag::tests::zero_node_graph_is_refused` | Test expects `Err(DagError::Empty)`; removing the empty-graph check yields a false pass |

> | `check_revision` in `src/port.rs` | Accept any revision without comparing to stored version | `dag::tests::stale_revision_refused` | Stale plan mutation is the primary state-safety invariant |

dag's remaining non-test catcher is `apply_tie_break` → "deterministic property (256 cases)", which is
a legitimately-specified property test with a stated denominator, not a placeholder.

### New finding of the same family (reduced severity)

Cross-checking every §11 catcher against its own §10 surfaces **two orphan test names** — concrete,
but declared nowhere else in their file, so no §10 row specs their inputs/expected output and no §13
DoD line gates them:

| File | Orphan test name | Only occurrence |
|---|---|---|
| `verify/BLUEPRINT.md` | `verify::tests::evidence_digest_mismatch_refused` | line 148 (§11) only |
| `notify/BLUEPRINT.md` | `notify::tests::zero_denominator_vacuous_proof_refused` | line 126 (§11) only |

This is strictly weaker than the ellipsis defect — a builder can create a test from a concrete name,
whereas `tests::...` is not a name at all — but the mutation floor still cannot be evaluated against
a test with no specified inputs or assertion. `notify::tests::failed_delivery_does_not_panic` is
*not* in this category: it is absent from §10's three-row table but is wired in §9 step 4 and gated
in §13, so it is fully specified.

`dag`, `intent`, `control`, `approval` have zero orphans — every §11 catcher resolves to a §10 row.

---

## Secondary: control/approval §11 file mapping — PARTIAL

**control §4** declares: `src/lib.rs`, `src/reducer.rs`, `src/scheduler.rs`, `src/supervisor.rs`,
`tests/recovery.rs`.

**control §11 rows 4-5** both mutate `reduce` in `src/reducer.rs` — **declared in §4**, so the literal
check passes. The imprecision: row 4 is "Move spawn before store commit" and row 5 is "Accept a stale
child generation" — both are child-lifecycle concerns that §4 assigns to `src/supervisor.rs`, yet are
attributed to `reducer.rs`. Their catchers are also prose descriptors ("kill-after-dispatch recovery
test", "late-child hidden test") rather than qualified names, though `tests/recovery.rs` is declared
and §10 does cover recovery. **PARTIAL** — declared file, wrong file for the described mutation.

**approval §4** declares: `src/lib.rs`, `src/grant.rs`, `src/check.rs`, `src/port.rs`,
`tests/approval.rs`.

**approval §11 `consume` rows** (lines 111 and 114) both reference **`src/check.rs`** — **declared in
§4**. The imprecision is on row 5 only: "Return success without durably writing the consumption
record" is durability, which §5 puts on the `ApprovalStore` trait and §9 step 4 explicitly implements
in `src/port.rs`. Row 2's read-only-fetch mutation is correctly `check.rs`. Both catchers
(`approval::tests::replay_refused_after_consume`) are real and declared in §10 and §13.
**PARTIAL** — declared file, one row's durability mutation belongs to `port.rs`.

Neither secondary issue blocks a builder: in both cases the named file exists in §4, the catcher is a
real test, and the described mutation is unambiguous enough to implement. They are precision defects,
not correctness defects.

---

## OVERALL: PARTIAL

All three named blockers are genuinely CLOSED with quoted evidence — verify's §9/§10/§11 secret path
is now internally consistent, intent's phantom `src/parse.rs` is gone tree-wide, and no BLUEPRINT.md
contains a `tests::...` placeholder — but two orphan test names (`verify::tests::evidence_digest_mismatch_refused`,
`notify::tests::zero_denominator_vacuous_proof_refused`) appear only in §11 with no §10 spec or §13
gate, and control/approval §11 keep two imprecise file attributions, so the tree is shippable to a
builder yet short of a clean PASS.
