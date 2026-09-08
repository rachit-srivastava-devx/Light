# Adversarial review — opus walkthrough

**Verdict: REJECT**

The two underlying silent failures are real, but the deliverable is not reproducible as written,
publishes no audit denominator, miscounts two defects plus a third defect as "Two defects", omits
four adjacent silent non-zero paths found by direct execution, and claims an end-to-end run without
providing enough commands or evidence to repeat it. The required full verifier is red.

## Contract checked

The requested backlog item named `opus-walkthrough` does not exist in the current
`handover/BACKLOG.md`. The only match is B8 line 94, where the walkthrough is evidence for the
unchecked item "Every non-zero exit must print a reason". B8 requires three fixes, an M9 detector
with a published denominator, mutation testing, and a green verifier. `tests/corpus/M9.sh` does not
exist and B8 remains `[ ]`.

This is a contract defect on its own: a reviewer cannot accept a deliverable against a named item
that is absent. I did not invent replacement criteria. I used the observable B8 claims plus every
runtime claim in the walkthrough.

## What the deliverable claims

1. A full `plan` -> refusal -> run without SOW -> SOW refusal -> SOW acceptance -> run -> status ->
   ledger verify -> tamper -> restore flow worked end to end.
2. `fleet run --task ""` exits 7 with zero bytes on both streams.
3. Tampered `fleet ledger verify` exits 8 silently; clean verification prints
   `verified checked=12 total=12`.
4. `fleet plan` with `FLEET_STATE` unset prints `intent`, `agent`, and `skills` before exiting on an
   environment fault.
5. There are "Two defects", followed by a third smaller defect, and a detector over every non-zero
   exit would catch all three.

## Runtime denominator

I retained **38 command executions** after correcting one reviewer-harness quoting error:

- 12 direct walkthrough/defect commands.
- 15 commands for an independent 12-row ledger, tamper, and restore check.
- 3 literal bare-`fleet` resolution checks.
- 8 adjacent hard-case checks.

Among those 38 executions, **17 returned non-zero**. **6 of 17 were silent** and **11 of 17 named
a reason**. The arithmetic is `6 + 11 = 17`; no case was dropped from that non-zero denominator.

All state directories were created with `mktemp -d`. The checkout binary was
`./keel/target/debug/fleet`. I did not run Git.

## What I ran and observed

| Command/scenario | Exit | stdout / stderr | Observation |
|---|---:|---:|---|
| Bare `fleet run --task ""` | 127 | 0 / 38 B | `fleet` is not on PATH. The deliverable's literal command is not runnable from this user shell. |
| Bare `fleet ledger verify` | 127 | 0 / 38 B | Same command-resolution failure. |
| Bare unset-state `fleet plan "add a --version flag to the cli"` | 127 | 0 / 38 B | Same command-resolution failure. |
| `FLEET_STATE=<tmp> ./keel/target/debug/fleet plan "add a --version flag to the cli"` | 0 | 499 / 0 B | Plan rendered three commands and published `denominator: 3`. |
| Unknown-intent `plan "make me a sandwich"` | 7 | 0 / 154 B | Useful refusal with three candidates. |
| `run` with a complete task before SOW acceptance | 7 | 0 / 290 B | Useful `SOW_NOT_ACCEPTED` refusal and exact SOW id. |
| Incomplete `sow --task "add a --version flag"` | 7 | 0 / 806 B | Names the missing citation and prints a corrected template. |
| Complete multiline SOW | 9 | 1604 / 194 B | `SOW_READY_AWAITING_REVIEW`; one 64-character id emitted. |
| `sow accept --id <captured-id>` | 0 | 125 / 0 B | `SOW_ACCEPTED` emitted. |
| Accepted `run` against the available target | 7 | 0 / 681 B | Refused because the target was dirty. No artifact and no successful end-to-end run were observed from the deliverable's instructions. |
| `status` after that flow | 0 | 2482 / 0 B | `1 of 1`; zero done, one failed `SOW_NOT_ACCEPTED`. |
| `ledger verify` after that flow | 0 | 27 / 0 B | `verified checked=5 total=5`. |
| Exact cited checkout form: `run --task ""` | 7 | 0 / 187 B | **Does not reproduce the claim.** It refuses first because `--repo` is missing. |
| Fully specified: `run --task "" --repo <target> --agent stub` | 7 | 0 / 0 B | **Reproduces the underlying silent empty-task defect.** |
| `env -u FLEET_STATE ... plan "add a --version flag to the cli"` | 3 | 55 / 264 B | Reproduces partial `intent`, `agent`, `skills` output before the environment reason. |
| 12 valid `ledger append` commands | 0 each | 72 / 0 B each | Created exactly 12 rows. |
| Clean 12-row `ledger verify` | 0 | 29 / 0 B | `verified checked=12 total=12`; arithmetic resolves exactly. |
| Tamper row 7 of 12, then `ledger verify` | 8 | 0 / 0 B | Reproduces the silent mismatch defect. |
| Restore the saved chain, then `ledger verify` | 0 | 29 / 0 B | Returns to `verified checked=12 total=12`. |

The full-flow claim is therefore **not independently verified**. The individual SOW and ledger
claims mostly resolve, but the document omits the binary path/PATH setup, `FLEET_STATE`, exact SOW
payload, target repository and clean-tree precondition, id capture, tamper command, restore command,
per-command exit codes, and artifact id. A prose sequence is not a runnable walkthrough.

## Hard cases the deliverable quietly drops

| Adjacent user-visible case | Exit | stdout / stderr | Classification |
|---|---:|---:|---|
| Fresh-state `ledger verify` | 6 | 0 / 0 B | Silent invariant failure. |
| Fresh-state `ledger count` | 6 | 0 / 0 B | Silent invariant failure. |
| Fresh-state `ledger dump` | 0 | 0 / 0 B | Empty output; not classified as a check, so not counted as a silent non-zero failure. |
| `ledger append --body not-json` | 7 | 0 / 0 B | Silent refusal. |
| `attest verify not-an-id` | 8 | 0 / 0 B | Silent mismatch. |
| `sow accept` with an unknown valid-shape id | 7 | 0 / 118 B | Names the reason. |
| Empty-store `status` | 0 | 2483 / 0 B | Explicitly says `empty store; 0 of 0 tasks`. |
| Empty-store `status --json` | 0 | 452 / 0 B | Explicit `empty: true`, `checked: 0`, `total: 0`. These are measured counts, not fabricated substitutes for `null`. |

The bounded audit therefore found **6 silent non-zero cases**, not two. Four were absent from the
deliverable: fresh ledger verify, fresh ledger count, invalid ledger JSON, and invalid attestation
id. The file's own arithmetic also fails: "Two defects" plus "A third" equals three, yet no
`3 of 3` denominator is published.

## Known-cheat audit

### Gate coverage says more than it proves

`bash tests/acceptance/p0.sh` passed, but `tests/acceptance/p0.sh:110-113` redirects the fully
specified empty-task command to `/dev/null` and asserts only exit 7 plus a receipt. It never asserts
that a human sees a reason.

`tests/acceptance/swarm.sh:249-270` reports `Q1 all 11 refusal surfaces are actionable`, but its run
probe is only `run --task`. That reaches missing-argument usage, which prints the word `fleet` and
passes the loose actionability matcher. It never reaches `run --task "" --repo <repo> --agent stub`,
which is silent. The published 11 is a selected-surface denominator, not every non-zero CLI surface.

This is the exact green-test/not-working-feature failure described by the task. No margin was
changed in this review, but the existing gate omits the hard branch and overstates its scope.

### Vacuous, zero/null, and self-documentation checks

- Fresh `ledger verify` and `ledger count` fail on zero inputs, so those checks do not pass
  vacuously; their defect is silence.
- Empty `status` reports `empty: true` and real zero counts. I found no `null`-to-zero fabrication in
  the deliverable.
- The M6 documentation detector did not produce a self-documentation false positive in this run;
  it timed out, so that property remains unverified here.
- The named backlog item is absent; the observable B8 item is correctly unchecked, not falsely done.

## Required independent verification

Command:

```bash
FLEET_MUTANTS=0 bash verify.sh
```

Observed top-level output:

```text
== fleet verify ==
  ok   fmt
  ok   clippy -D warn
  ok   unit tests
  ok   acceptance builds
  ok   cargo-deny
  ok   cargo-audit
  ok   secrets
  ok   acceptance
  ok   readme
  ok   swarm
  ok   policy
  SKIPPED WITH A REASON mutants (set FLEET_MUTANTS=1 - full pass ~24min)
  ok   attest-smoke
  ok   pytest
  ok   detectors
  FAIL corpus (see var/verify.log)
-- 14 passed, 1 failed, 1 skipped (denominator: 16 stages) --
exit=6
```

The red corpus evidence was:

```text
detector-integrity: 105 detectors match the manifest (denominator: 105)
M2: 34430 files in the tree (>15000). Build artifacts are almost certainly inside the repo;
    set CARGO_TARGET_DIR outside it. Detectors walk the tree and will stall (D30).
TIMEOUT: A1 A3 A4 A8 B10 C1 C11 C12 C19 C2 C22 C24 C26 C3 C6 C8 C9 M6
         S4 S6 S9 T1 T15 T20 T5 T6
DENOMINATOR checked=34 total=34 excluded=69 caught=27
```

The timeout list contains 26 detectors; `M2` is the 27th caught failure. Corpus arithmetic checks:
`34 checked + 69 excluded = 103 executed detector files`, plus skipped runner files `run.sh` and
`_selftest.sh`, matching the 105-file integrity denominator.

Standalone required P0 command:

```bash
bash tests/acceptance/p0.sh
```

Observed exit `0` and output:

```text
== P0 acceptance ==
  ok   A1 fleet run exits 0
  ok   A2 run prints an artifact id
  ok   B1 artifact exists in the store
tests/acceptance/p0.sh: line 53: /var/folders/_x/39gv9nf93lbfh7p1fvzl1nn80000gn/T//fleet-p0.GPDLAN/artifacts/0094d2237b98a12d0c45d811d904ae86d3b73f181c3f460e2f71cb297942bded: Permission denied
  ok   B2 artifact mode is 0444
  ok   B3 artifact is not writable
  ok   B4 artifact id == its content hash (independent oracle)
  ok   C1 attest verify exits 0
  ok   C2 attestation written
  ok   C3 in-toto shape: subject digest == artifact id, predicateType correct
  ok   C4 tampered attestation -> exit 8
  ok   D1 no ledger path leaked into a worker env record
  ok   E1 ledger chain verifies
  ok   E2 chain valid after 20 concurrent appends
  ok   E3 no rows lost (count=29)
  ok   E4 no prev_hash reuse (THE assertion the row count misses)
  ok   F1 empty task refused with exit 7
  ok   F2 the refusal WROTE A RECEIPT (S1: 4 refusals -> 0 receipts)
  ok   G1 blake3 is a RUNTIME dependency, not hand-rolled
  ok   G2 no hand-rolled hash permutation in the tree
  ok   G3 exactly one implementation (A4: one implementation per concept)
  ok   H:--help exits 0 with real output (3640 bytes)
  ok   H:--version exits 0 with real output (82 bytes)
  ok   H:doctor exits 0 with real output (401 bytes)
  ok   H:bare invocation prints usage
  ok   H:'ratchet:show' is reachable (not a blanket refusal)
  ok   H:'console:--help' is reachable (not a blanket refusal)
  ok   H:'oracle' explains its refusal (67 bytes)
  ok   H:'adjudicate' explains its refusal (78 bytes)
  ok   H:'ratchet' explains its refusal (27 bytes)
  ok   H:'graph' explains its refusal (63 bytes)
  ok   H:'mcp' explains its refusal (78 bytes)
  ok   I1 missing FLEET_STATE exits 3
  ok   I2 env fault explains itself (263 bytes)
  ok   I3 doctor still reports with FLEET_STATE unset (444 bytes)
== 34 passed, 0 failed ==
```

P0 green does not override the direct silent-output evidence or the red full verifier.

## Exactly what must change before acceptance

1. Restore/add a backlog item explicitly named `opus-walkthrough`, or identify B8 as the contract;
   the review target and acceptance criteria must resolve unambiguously.
2. Rewrite the walkthrough as an executable transcript: explicit checkout binary or PATH setup,
   isolated `FLEET_STATE`, clean target-repo precondition, exact multiline SOW, id capture, every
   command, immediate exit code, stdout/stderr byte counts, artifact id, exact tamper, and restore.
3. Correct the empty-task reproduction to include required `--repo` and `--agent` arguments. State
   separately that the shorthand as written emits 187 stderr bytes and tests the wrong branch.
4. Publish a checkable denominator. At minimum classify all 17 non-zero executions in this audit,
   including the 6 silent cases, and state the bounded surface/exclusions. Do not call it "Two
   defects" while listing three or omit the four adjacent silent cases above.
5. Replace the unsupported end-to-end statement with current evidence, or rerun on a named clean
   target and record exit 0, artifact id, status denominator, clean/tampered/restored ledger counts,
   and both-stream output.
6. Add the B8 M9 detector and assert output on fully specified semantic refusal/mismatch paths, not
   missing-argument usage. Publish its complete surface denominator and mutation-test both pass and
   fail directions.
7. Re-run `FLEET_MUTANTS=0 bash verify.sh` in a state where the 34,430-file tree/corpus timeouts are
   resolved. Acceptance requires the final summary and exit `0`; the observed result is exit `6`.
