# Adversarial review: opus walkthrough

Review ID: `OPUS-WALKTHROUGH-ADVERSARIAL-20260826-CODEX-PRIMARY6`  
Reviewed: `docs/delta.d/opus-walkthrough.md` on 2026-08-26  
No direct `git` command was run by this review. I did not edit the deliverable, `docs/DELTA.md`,
`handover/BACKLOG.md`, or `keel/`; this report is the only intentional review artifact created in
the repository.

## Contract

There is no literal `opus-walkthrough` item in `handover/BACKLOG.md`. The only matching contract is
B8, “Every non-zero exit must print a reason,” at `handover/BACKLOG.md:92-106`; B8 is still
unchecked. B8 requires the three reported paths to be fixed, `tests/corpus/M9.sh` with a published
denominator, mutation evidence, and a green `FLEET_MUTANTS=0 bash verify.sh`.

## What was claimed

The deliverable claims:

1. A full `plan` -> SOW refusal -> SOW acceptance -> `run` -> `status` -> ledger verify -> tamper ->
   restore flow worked end to end.
2. `fleet run --task ""` returns exit 7 with zero output.
3. Tampered `fleet ledger verify` returns exit 8 with zero output.
4. `fleet plan` with `FLEET_STATE` unset prints `intent:`, `agent:`, and `skills:` before its
   environment fault.
5. These are two defects plus a third smaller defect, and a detector should require a reason on
   every non-zero exit.

## Commands actually run

The installed `fleet` command is not available in a fresh shell: `command -v fleet` returned 1 and
`fleet --version` returned 127. I used the existing release binary at
`keel/target/release/fleet` and, where appropriate, prepended that directory to `PATH`.

| Command / setup | Exit | Captured result |
|---|---:|---|
| `fleet run --task ""` with the release directory on `PATH` and a temporary `FLEET_STATE` | 7 | stdout 0 bytes; stderr 187 bytes containing `--repo is required` and usage |
| `fleet run --repo "$PWD" --agent stub --task ""` with temporary state | 7 | stdout 0 bytes; stderr 0 bytes |
| `env -u FLEET_STATE fleet plan "change the README"` | 3 | stdout 55 bytes: the three plan lines; stderr 264 bytes explaining the missing state |
| copied valid ledger, `fleet ledger verify` | 0 | `verified checked=9 total=9` |
| same copied ledger after changing one byte, `fleet ledger verify` | 8 | stdout 0 bytes; stderr 0 bytes |
| fresh state, `fleet ledger verify` | 6 | stdout 0 bytes; stderr 0 bytes |
| fresh state, `fleet ledger append` | 7 | stdout 0 bytes; stderr 0 bytes |
| fresh state, `fleet attest verify garbage-not-a-hash` | 8 | stdout 0 bytes; stderr 0 bytes |
| fresh state, `fleet impact` | 7 | stdout 0 bytes; stderr 0 bytes |
| fresh state, `fleet status --json` | 0 | `checked=0`, `total=0`, `empty=true` |

The two central silent failures are therefore real, and the unset-state partial output is real. The
literal empty-task command in the document is not the claimed product path: it reaches argument
validation and emits usage because `--repo` and `--agent` are absent. Only the complete command
reproduces the silent empty-task refusal.

I also exercised the success sequence in copied temporary state. SOW creation returned 9 and
acceptance returned 0. The subsequent `run` returned 7 because the copied target repository was
already dirty; it emitted 521 bytes explaining the clean-tree requirement. No artifact was
produced. This does not prove the clean happy path is broken, but it proves the document does not
contain enough setup to reproduce its “works end to end” claim.

## Arithmetic and coverage

The document has no `checked/total` denominator for either the walkthrough or the proposed detector.
“Two defects” plus “a third” describes three observations, but it does not define the population
checked or classify omitted/untriggerable failure surfaces.

The adjacent probe found at least five additional silent non-zero surfaces: empty-ledger verify,
ledger append with no arguments, invalid attestation, impact without a symbol, and the valid
empty-task refusal. The empty `status --json` path is worse in a different way: it exits 0 while
publishing `checked=0 total=0`, which is a vacuous pass under the repository’s own denominator law.

The repository currently has 103 actual corpus detectors plus the `run.sh` and `_selftest.sh`
helpers, and 105 manifest lines. `tests/corpus/M9.sh` is absent. The required verifier’s corpus
log reports `checked=34 total=34 excluded=69 caught=19`; `34 + 69 = 103`, so that population is
internally explainable only after explicitly excluding the two helpers. The walkthrough publishes
none of this accounting.

## Required verifier evidence

`bash tests/acceptance/p0.sh` passed with exit 0:

```text
== 34 passed, 0 failed ==
```

That targeted suite does not assert the silent failure paths.

The required independent command was run exactly:

```text
$ FLEET_MUTANTS=0 bash verify.sh
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
exit 6
```

`var/verify.log` names timeout failures including `A1`, `A3`, `A4`, `A8`, `C2`, `C6`, `C11`,
`C22`, `C26`, `S4`, `S6`, `S9`, `T1`, `T5`, `T6`, `T15`, and `T20`, and ends with
`DENOMINATOR checked=34 total=34 excluded=69 caught=19`. Mutation testing was skipped by the
requested `FLEET_MUTANTS=0` invocation, so it is not mutation evidence.

## Findings

### F1 — REJECT: the acceptance contract is absent and its nearest item is incomplete

No standalone `opus-walkthrough` backlog item exists. If B8 is intended to govern this fragment,
the artifact does not satisfy B8: M9 is absent, no detector denominator or mutation result is
published, and the required verifier is red.

### F2 — REJECT: the primary repro command contradicts its own observation

`docs/delta.d/opus-walkthrough.md:11` cites `fleet run --task ""` but omits required arguments.
That command produced 187 bytes of usage output in the prepared shell. The silent result requires
`--repo` and `--agent stub`, plus a usable state directory. The document must not leave a shorthand
command that produces a different failure mode.

### F3 — REJECT: the end-to-end success claim has no reproducible evidence

`docs/delta.d/opus-walkthrough.md:3-6` gives no exact task text, binary identity, state path,
repository setup, clean-tree precondition, command exit codes, artifact ID, status denominator,
tamper edit, or restore command. The replay reached SOW acceptance but stopped at the dirty-target
refusal. “The flow works end to end” is therefore unsupported by this deliverable.

### F4 — REJECT: the proposed “every non-zero exit” detector is not scoped or complete

The document reports only three paths. Fresh-state probing found additional silent non-zero paths,
including empty-ledger verification and invalid attestation. It also omits the zero-input
`status --json` vacuous pass. A detector that only checks the three prose examples would not enforce
the stated property.

### F5 — REJECT: denominator and classification are missing

There is no walkthrough `checked/total`, no detector `checked/total`, and no list of excluded or
untriggerable surfaces. The document cannot demonstrate that hard cases were checked rather than
dropped. This directly violates the repository law and B8’s acceptance wording.

### F6 — REJECT: the required gate is red

The full verifier exited 6 because the corpus stage failed. Its final stage arithmetic is
checkable, `14 + 1 + 1 = 16`, but it is not green. P0 being green is not a substitute for the
required full gate, and `FLEET_MUTANTS=0` explicitly provides no mutation pass.

## Verdict

**REJECT**

## Exact changes required before acceptance

1. Add an explicit `opus-walkthrough` backlog contract, or explicitly identify B8 as the contract
   for this artifact.
2. Rewrite the walkthrough as a self-contained transcript using the complete empty-task command,
   explicit binary/setup, isolated `FLEET_STATE`, a clean disposable Git target, exact task/SOW
   text, captured stdout/stderr, exit codes, artifact/status/ledger counts, the actual tamper edit,
   and the restore command.
3. Fix the valid-argument empty-task refusal, tampered-ledger mismatch, and unset-state partial
   output. Audit and fix the other reachable silent non-zero paths, including empty-ledger and
   attestation mismatch, or explicitly document their contract.
4. Add `tests/corpus/M9.sh` for every reachable refusable surface. It must publish a non-zero
   `checked/total` denominator, fail on empty input, and avoid matching its own documentation.
5. Mutation-test M9 in both refusal and non-refusal directions, update detector integrity as
   required, and publish the real caught/total result. Do not count skipped mutants as evidence.
6. Rerun `FLEET_MUTANTS=0 bash verify.sh` and require exit 0 with no failed stages.
