# Adversarial review: opus walkthrough

Date: 2026-08-25
Deliverable: docs/delta.d/opus-walkthrough.md
Boundary: no git commands were run. The deliverable, DELTA.md, BACKLOG.md, and keel/ were not edited. This report is the only file created by this review.

## Verdict

REJECT

The two silent failure paths are real, and the partial-plan output is real, but this is not acceptance evidence. The primary empty-task command is incomplete, there is no walkthrough denominator, M9 is absent, and the required verifier is red.

## Contract

There is no standalone opus-walkthrough heading in handover/BACKLOG.md. The only occurrence is B8's reference at lines 94-96. Treating B8 as the nearest available contract, its acceptance criteria at lines 101-106 require all three paths fixed, tests/corpus/M9.sh with a published denominator, mutation testing, and a green verify.sh. B8 remains unchecked.

## What was claimed

- A full plan -> refusal -> run -> SOW -> accepted SOW -> run -> status -> ledger verify -> tamper -> restore flow works end to end.
- fleet run --task "" exits 7 with zero bytes on both streams.
- fleet ledger verify on a tampered chain exits 8 with no output.
- fleet plan with FLEET_STATE unset emits three plan lines before exit 3.
- A detector should assert that every non-zero exit writes a reason.

## What I actually ran

All runs used keel/target/release/fleet and isolated temporary state directories unless noted.

### Empty task

The command exactly as written was run:

    keel/target/release/fleet run --task ""

Result: exit 7, stdout 0 bytes, stderr 187 bytes. It printed:

    fleet: run: --repo is required.
    usage: fleet run --task <T> --repo <P> --agent <stub|env-probe|freelane|claude|codex>

The documented command is therefore wrong as a reproduction. The complete command was also run:

    keel/target/release/fleet run --task "" --repo "/Users/rachitsrivastava/youtube/Principal Engineering/fleet-rs" --agent stub

Result: exit 7, stdout 0 bytes, stderr 0 bytes. This reproduces the actual user-facing defect. A receipt is written, but the user receives no reason. The empty-task branch in run_with_evidence appends a receipt and returns without a diagnostic.

### Plan with missing state

    env -u FLEET_STATE keel/target/release/fleet plan "add a --version flag"

Result: exit 3, stdout 55 bytes, stderr 264 bytes.

stdout:

    intent: implement a change
    agent: builder
    skills: rust

stderr then reports that FLEET_STATE is not set. This confirms the partial-output defect. plan_command prints those fields before calling the state-dependent route at keel/fleet/src/main.rs:2646-2712.

### SOW refusal and acceptance

    keel/target/release/fleet sow --task "add a --version flag"

Result: exit 7, stdout 0, stderr 806. It named the missing challenge citation and emitted a corrected template.

A complete SOW was then submitted. Result: exit 9 with a 64-character ID on stderr. The cited acceptance command was run with that real ID:

    keel/target/release/fleet sow accept --id <captured-id>

Result: exit 0, stdout 125, stderr 0, including:

    SOW_ACCEPTED id=<captured-id> by=rachitsrivastava at=2026-08-25T00:35:29Z

The subsequent empty-task run still returned exit 7 with zero bytes on both streams.

A non-empty bypassed run against the shared checkout was attempted:

    FLEET_SOW_BYPASS=1 keel/target/release/fleet run --task "add a --version flag" --repo "/Users/rachitsrivastava/youtube/Principal Engineering/fleet-rs" --agent stub

Result: exit 7, stdout 0, stderr 638. The target checkout was refused as dirty. This does not establish the claimed successful end-to-end run; the document supplies no disposable clean-target setup that an independent reviewer can replay.

### Ledger tamper and restore

A fresh isolated state was populated by the SOW and refusal receipts. One field in $FLEET_STATE/ledger/chain.jsonl was changed from EMPTY_TASK to EMPTY_TASX, then this command was run:

    keel/target/release/fleet ledger verify

Tampered result: exit 8, stdout 0, stderr 0. This reproduces the second silent failure.

The original chain was restored. The same command returned exit 0, stdout 27 bytes:

    verified checked=3 total=3

ledger_verify calls verify_rows(&rows)? before its success println at keel/fleet/src/main.rs:2995-3000, so mismatch errors return before any failure diagnostic.

### Additional empty-store check

As a separate known-cheat check:

    FLEET_STATE=<fresh-empty-dir> keel/target/release/fleet status --json

Result: exit 0, with checked=0, total=0, empty=true. This is B14 rather than the walkthrough itself, but it is another vacuous green result against the repository law that zero checked inputs must fail or be explicitly classified.

## Arithmetic and coverage

The walkthrough reports three problematic cases, but it does not state checked/total, enumerate all attempted refusal surfaces, or classify omitted and untriggerable surfaces. The 12/12 value in the success-path prose is a ledger-row count, not walkthrough coverage.

The repository contains 105 shell corpus files, including run.sh and _selftest.sh. M9 is absent:

    bash tests/corpus/M9.sh

Result: exit 127; bash: tests/corpus/M9.sh: No such file or directory.

No M9 denominator or mutation result exists. The proposed word every is not measured.

## Required verifier

The required command was run to completion:

    FLEET_MUTANTS=0 bash verify.sh

The real terminal result was:

    == fleet verify ==
      .... fmt
      ok   fmt
      .... clippy -D warn
      ok   clippy -D warn
      .... unit tests
      ok   unit tests
      .... acceptance builds
      ok   acceptance builds
      .... cargo-deny
      ok   cargo-deny
      .... cargo-audit
      ok   cargo-audit
      .... secrets
      ok   secrets
      .... acceptance
      ok   acceptance
      .... readme
      ok   readme
      .... swarm
      ok   swarm
      .... policy
      ok   policy
      SKIPPED WITH A REASON mutants (set FLEET_MUTANTS=1 - full pass ~24min)
      .... attest-smoke
      ok   attest-smoke
      .... pytest
      ok   pytest
      .... detectors
      ok   detectors
      .... corpus
      FAIL corpus (see var/verify.log)
    -- 14 passed, 1 failed, 1 skipped (denominator: 16 stages) --

Process exit code: 6.

var/verify.log contains these red results:

    TIMEOUT C11.sh exceeded 30s -- treated as FAILED
    detector timed out
    TIMEOUT C2.sh exceeded 30s -- treated as FAILED
    detector timed out
    TIMEOUT C22.sh exceeded 30s -- treated as FAILED
    detector timed out
    TIMEOUT C6.sh exceeded 30s -- treated as FAILED
    detector timed out
    TIMEOUT M6.sh exceeded 30s -- treated as FAILED
    detector timed out
    TIMEOUT S6.sh exceeded 30s -- treated as FAILED
    detector timed out
    TIMEOUT S9.sh exceeded 30s -- treated as FAILED
    detector timed out
    TIMEOUT T1.sh exceeded 30s -- treated as FAILED
    detector timed out
    TIMEOUT T15.sh exceeded 30s -- treated as FAILED
    detector timed out
    DENOMINATOR checked=34 total=34 excluded=69 caught=12

The corpus arithmetic is internally consistent: 34 checked plus 69 excluded equals 103 non-helper detectors. It is still red, with 12 caught findings and multiple timeout failures. It provides no M9 evidence.

## Findings

1. REJECT: the named contract is absent. There is no standalone opus-walkthrough acceptance item; B8 is the only binding-looking item and remains unchecked.
2. REJECT: the primary reproduction is incorrect as documented. fleet run --task "" reports missing --repo; the silent defect requires --repo and --agent.
3. REJECT: the documented product defects remain. The valid empty-task and tampered-ledger paths return non-zero with zero user-visible bytes, and the partial plan still emits output before its environment refusal.
4. REJECT: the walkthrough has no denominator. Three observed cases do not support the proposed every-refusable-surface detector, and omitted hard cases are not classified.
5. REJECT: B8 is not met. M9 is absent, no mutation test is recorded, and the required verifier exits 6 with corpus red.

## Exactly what must change

1. Add a real opus-walkthrough backlog item, or explicitly bind this artifact to B8 and state that mapping.
2. Correct the empty-task transcript to include all required flags and isolate the target state and repository. Keep the shorthand argument-validation result separate if it remains documented.
3. Fix all three user-facing paths so every non-zero result writes at least one line naming its reason: empty task, ledger mismatch, and partial-plan environment failure.
4. Reproduce the accepted run on a disposable clean target with exact setup, artifact, status, attestation, ledger, tamper, restore commands, exit codes, and output bytes.
5. Add tests/corpus/M9.sh covering every refusable surface, fail when zero surfaces are checked, publish checked/total, and include both reason and no-reason mutation directions without matching its own documentation.
6. Rerun mutation evidence and FLEET_MUTANTS=0 bash verify.sh; acceptance requires final exit code 0 with the full stage denominator.
