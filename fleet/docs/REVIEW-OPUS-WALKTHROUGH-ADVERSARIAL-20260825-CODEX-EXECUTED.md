# Adversarial review: opus-walkthrough

Reviewed: `docs/delta.d/opus-walkthrough.md` on 2026-08-25.

Verdict: **REJECT**.

The walkthrough contains one real current defect (tampered-ledger verification is silent) and
one real partial-output defect (`fleet plan` with no `FLEET_STATE`). Its first defect is not
reproducible from the command as written: `fleet run --task ""` omits required `--repo` and
`--agent` arguments and reaches the usage path. With the required arguments supplied, the empty
task is still silently refused. The referenced acceptance work is also incomplete: B8 is
unchecked, M9 is absent, mutation testing was skipped, and the required verifier is red.

## Contract

There is no standalone `opus-walkthrough` item in `handover/BACKLOG.md`. The only matching
reference is B8 at lines 94-106. I therefore used B8 as the nearest available contract. It
requires all three user-facing paths to be fixed, `tests/corpus/M9.sh` with a published
denominator and mutation evidence, and a green `verify.sh`.

## What was claimed

The deliverable claims:

1. A full plan/refusal/SOW/accept/run/status/ledger/tamper/restore flow works end to end.
2. `fleet run --task ""` exits 7 with zero bytes on both streams.
3. Tampered `fleet ledger verify` exits 8 with no output.
4. Unset `FLEET_STATE` causes `fleet plan` to print `intent`, `agent`, and `skills` before the
   environment refusal.
5. A detector should check that every non-zero exit emits a reason.

The document publishes no checked/total denominator for the walkthrough, no per-command exit
matrix, no artifact/state setup, and no evidence for the successful run itself.

## Commands actually run

The `fleet` command was not installed on `PATH`: `fleet --version` exited 127. I then built the
documented release binary from this checkout and ran it directly as
`target-shared/release/fleet`, with temporary state directories.

| Command/case | Exit | stdout bytes | stderr bytes | Observation |
|---|---:|---:|---:|---|
| `fleet run --task ""` exactly as written | 7 | 0 | 187 | Usage says `--repo` is required; this does not test empty-task handling. |
| `fleet run --task "" --repo "$PWD" --agent stub` | 7 | 0 | 0 | The valid-argument empty-task case is silently refused, confirming the underlying defect. |
| clean copied ledger: `fleet ledger verify` | 0 | 27 | 0 | `verified checked=9 total=9`. |
| same ledger after changing row 2 | 8 | 0 | 0 | Tamper defect reproduced exactly. |
| restored copied ledger: `fleet ledger verify` | 0 | 27 | 0 | `verified checked=9 total=9`; both directions work, but the failure has no reason. |
| `fleet plan "add a --version flag to the cli"` | 0 | 499 | 0 | Plan is printed; route is unavailable and explained. |
| `fleet plan "make me a sandwich"` | 7 | 0 | 154 | Refusal names three candidates. |
| `env -u FLEET_STATE fleet plan "add a --version flag to the cli"` | 3 | 55 | 264 | `intent`, `agent`, `skills` leak to stdout before the environment reason. |
| literal bad-repo run from the walkthrough | 3 | 0 | 68 | Environment refusal names the missing repository. |
| literal incomplete `fleet sow --task "add a --version flag"` | 7 | 0 | 806 | Refusal and corrected template are printed. |
| literal placeholder `fleet sow accept --id fcea10dfc63d8f37` | 7 | 0 | 62 | Invalid-id refusal is printed. |
| `fleet attest verify 0094d2237b98a12d` | 8 | 0 | 0 | Mismatch path is also silent. |

The initial `fleet`-on-`PATH` failures were shell environment failures (rc 127), not product
results. All Fleet observations above use the current checkout's freshly built binary.

## Arithmetic and coverage

- The walkthrough says “two defects” and then adds a “third” case: three failure cases are
  discussed, but no denominator says how many command cases were checked or how many passed.
- The successful full-flow claim cannot be recomputed: it gives no exact task text, repo path,
  SOW id, artifact id, state snapshot, command exit codes, or ledger counts.
- `FLEET_MUTANTS=0 bash verify.sh` completed with this real result:

  ```text
  FAIL corpus                     (see var/verify.log)
  -- 14 passed, 1 failed, 1 skipped (denominator: 16 stages) --
  ```

  The verifier exited **6**. The mutants stage was explicitly skipped because
  `FLEET_MUTANTS=0`.
- The corpus log reports `DENOMINATOR checked=34 total=34 excluded=69 caught=24`; 34 + 69 =
  103. Detector integrity separately reports 105 manifest detectors, so the corpus denominator
  needs to state which two helper entries are outside the 103-case scope.
- The corpus red output includes 22 timeout failures: `A1 A3 A4 A8 C1 C11 C2 C22 C24 C26 C3
  C6 C8 C9 S4 S6 S9 T1 T15 T20 T5 T6`. A timeout is treated as failed; this is not green
  evidence.

## Acceptance checks

`bash tests/corpus/M9.sh` exited **127**:

```text
bash: tests/corpus/M9.sh: No such file or directory
```

No M9 denominator or M9 mutation result exists. The required detector is not present, and the
full verifier did not pass.

## Required changes for acceptance

1. Add or restore an explicit `opus-walkthrough` backlog item, or explicitly rename this review
   target as B8 so the contract is not inferred.
2. Correct the walkthrough command and evidence: use required arguments for the empty-task case,
   publish exact exit/stream results, and provide reproducible setup and artifacts for the claimed
   successful flow.
3. Fix every silent non-zero user path found here, at minimum valid-argument empty-task refusal,
   tampered-ledger mismatch, and attest mismatch; decide and document the empty-ledger rc 6
   diagnostic as well.
4. Fix the partial `fleet plan` output by checking the environment before printing the plan, or
   document and contract the partial-output behavior explicitly.
5. Add `tests/corpus/M9.sh` covering every refusable surface. It must publish `checked/total`,
   fail on zero checked inputs, and assert a reason line for every non-zero exit. Mutation-test
   the reason assertions in both directions and record the real result.
6. Rerun `FLEET_MUTANTS=0 bash verify.sh` to a final exit 0 with no failed stage; do not treat the
   skipped mutants stage or the current corpus denominator as mutation evidence.
