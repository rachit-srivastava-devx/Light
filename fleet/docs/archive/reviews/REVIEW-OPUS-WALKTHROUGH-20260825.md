# Adversarial review: opus walkthrough

Date: 2026-08-25

Verdict: **REJECT**

## Contract check

The requested acceptance item `opus-walkthrough` does not exist in
`handover/BACKLOG.md`. The only match is the B8 reference at lines 94-96. There
is therefore no identifiable acceptance contract for this deliverable. B8 is
still open and requires the three defects to be fixed, M9 to exist with a
published denominator and mutation evidence, and `verify.sh` to be green.

## What the document claims

- Lines 3-6 claim a complete `plan -> refusal -> run -> SOW -> run -> status ->
  ledger verify -> tamper -> restore` flow works end to end, and that the SOW
  refusal gives useful correction guidance.
- Lines 11-14 claim `fleet run --task ""` exits 7 silently and that tampered
  `fleet ledger verify` exits 8 silently, while the success path reports
  `verified checked=12 total=12`.
- Lines 16-18 claim `fleet plan` with `FLEET_STATE` unset emits partial plan
  output before the environment refusal.
- Lines 20-22 propose a detector for every non-zero exit.

## Commands actually run

All runs used the freshly built `keel/target/release/fleet`; `fleet` was not on
PATH. The build command from the handover completed with exit 0 in 4m27s.

| Case | Exit | stdout bytes | stderr bytes | Observation |
|---|---:|---:|---:|---|
| `fleet plan` with no args | 7 | 0 | 41 | Usage is printed; not the documented silent case. |
| `fleet plan "make a code change"` with `FLEET_STATE` unset | 3 | 55 | 264 | Reproduces partial `intent`, `agent` output before the environment fault. |
| `fleet plan "add a --version flag"` with isolated state | 0 | 499 | 0 | Emits a 3-command plan and `commands: 3 planned (denominator: 3)`; route is unavailable but planning completes. |
| Exact `fleet run --task ""` | 7 | 0 | 187 | Does not reach empty-task validation; current CLI refuses first because `--repo` is required. |
| Valid empty-task form: `fleet run --task "" --repo . --agent stub` | 7 | 0 | 0 | Reproduces the underlying silent refusal exactly. |
| `fleet run --task <task> --repo . --agent stub` before SOW | 7 | 0 | 290 | Refusal names the SOW id and accept command. |
| `fleet sow --task "add a --version flag"` | 7 | 0 | 806 | Refusal names missing challenge citation and prints a corrected template. |
| Re-run with the printed corrected template | 9 | 1502 | 194 | SOW is emitted; `SOW_READY_AWAITING_REVIEW` id is printed. |
| `fleet sow accept --id <printed-id>` | 0 | 125 | 0 | Acceptance receipt is emitted. |
| Run after acceptance against this shared checkout | 7 | 0 | 397 | Refused because the target repository has uncommitted changes. The claimed end-to-end run could not complete here. |
| `fleet status --json` on a fresh state | 0 | 452 | 0 | Returns `checked: 0`, `total: 0`, `empty: true`; this is a vacuous clean exit. |
| `fleet ledger verify` on an empty state | 6 | 0 | 0 | Silent invariant failure. |
| 12 valid ledger appends, then `fleet ledger verify` | 0 | 29 | 0 | `verified checked=12 total=12`. |
| Same 12-row chain after changing one body field | 8 | 0 | 0 | Silent mismatch, reproducing the document's tamper claim. |
| Restored 12-row chain | 0 | 29 | 0 | `verified checked=12 total=12`. |

The SOW refusal claim is supported. The two underlying silent failure claims
are supported only after correcting the first command to the current required
CLI syntax. The literal command written in the document is stale and does not
exercise the claimed path.

## Arithmetic and omissions

The document reports three defects, but publishes no denominator for the
walkthrough: no number of commands/cases attempted, no success/refusal split,
and no count of hard cases considered or excluded. The only checkable ratio is
the ledger's `12/12`; it is not a denominator for the overall flow. The
document also omits the fresh-store `status --json` result of `0/0` with exit 0,
which is a known project invariant violation and directly contradicts an
unqualified “works end to end” claim.

## Independent verifier

Command run exactly as required:

```text
$ FLEET_MUTANTS=0 bash verify.sh
VERIFY_RC=6
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
```

The corpus log's red evidence was:

```text
TIMEOUT A4.sh exceeded 30s -- treated as FAILED
detector timed out
TIMEOUT C11.sh exceeded 30s -- treated as FAILED
detector timed out
TIMEOUT C2.sh exceeded 30s -- treated as FAILED
detector timed out
TIMEOUT C22.sh exceeded 30s -- treated as FAILED
detector timed out
TIMEOUT C6.sh exceeded 30s -- treated as FAILED
detector timed out
TIMEOUT S6.sh exceeded 30s -- treated as FAILED
detector timed out
TIMEOUT S9.sh exceeded 30s -- treated as FAILED
detector timed out
TIMEOUT T1.sh exceeded 30s -- treated as FAILED
detector timed out
TIMEOUT T15.sh exceeded 30s -- treated as FAILED
detector timed out
M2: 32060 files in the tree (>15000). Build artifacts are almost certainly inside the repo;
    set CARGO_TARGET_DIR outside it. Detectors walk the tree and will stall (D30).
DENOMINATOR checked=34 total=34 excluded=69 caught=12
```

This is not a green full-gate result. The verifier's stage denominator is
16, with 14 passed, 1 failed, and 1 skipped. The corpus also publishes its
34/34 checked denominator but fails on timeouts.

## Required changes for acceptance

1. Add an actual `opus-walkthrough` item to `handover/BACKLOG.md`, or explicitly
   state that B8 is the governing contract. Its criteria must require exact
   commands, exit codes, stdout/stderr evidence, a total-case denominator, and
   classification of every tested hard case.
2. Correct the documented empty-task command to include the required
   `--repo` and `--agent` arguments, or update the CLI contract. Preserve the
   valid reproduction showing exit 7 with zero bytes on both streams.
3. Add the omitted fresh-store `status --json` result and classify whether
   `checked=0,total=0,exit=0` is intentional or an invariant failure. Do not
   call the complete flow working while that case is unclassified.
4. Re-run the accepted-SOW path on a clean target repository and include the
   actual `run`, `status`, `ledger verify`, tamper, and restore transcripts.
5. Add the promised detector/acceptance coverage for non-zero output, publish
   its denominator and mutation result, and make the independent verifier
   finish green. The current evidence is `14 passed, 1 failed, 1 skipped`.

