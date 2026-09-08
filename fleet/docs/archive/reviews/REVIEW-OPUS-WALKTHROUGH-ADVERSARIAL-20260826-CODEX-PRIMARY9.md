# Adversarial review: opus walkthrough

Review ID: `OPUS-WALKTHROUGH-ADVERSARIAL-20260826-CODEX-PRIMARY9`  
Reviewed: `docs/delta.d/opus-walkthrough.md` on 2026-08-26  
No Git command was run. Only this review file was created.

## Contract

There is no standalone `opus-walkthrough` item in `handover/BACKLOG.md`:

```text
$ rg -n '^## .*opus-walkthrough' handover/BACKLOG.md; rc=$?; echo "rc=$rc"
rc=1
```

The nearest binding contract is B8 (`handover/BACKLOG.md:92-106`), which references this
walkthrough and requires all three failure paths to be fixed, `tests/corpus/M9.sh` with a
published denominator, mutation testing, and a green `verify.sh`.

## What was claimed

The deliverable claims that a complete `plan` → SOW → `run` → `status` → ledger verify → tamper →
restore flow worked end to end, and reports three user-visible defects:

1. Empty task run exits 7 with no output.
2. Tampered ledger verification exits 8 with no output.
3. Unset-state planning emits partial output before the environment failure.

It also cites a success result of `verified checked=12 total=12`, but supplies no task text, SOW
id, binary path, state directory, repository path, agent, captured transcript, or denominator for
the walkthrough itself.

## Fresh commands and results

All commands below used a fresh temporary `FLEET_STATE` and the checkout binary
`keel/target/debug/fleet` unless stated otherwise.

### The literal empty-task command is incomplete

```text
$ env FLEET_STATE="$STATE" fleet run --task ""
rc=127
stdout_bytes=0 stderr_bytes=38
stderr: env: fleet: No such file or directory

$ env FLEET_STATE="$STATE" ./keel/target/debug/fleet run --task ""
rc=7
stdout_bytes=0 stderr_bytes=187
stderr: fleet: run: --repo is required.
        usage: fleet run --task <T> --repo <P> --agent <stub|env-probe|freelane|claude|codex>
```

The command as written in the deliverable does not reach the empty-task branch. Adding the
required arguments produces the claimed defect:

```text
$ env FLEET_STATE="$STATE" ./keel/target/debug/fleet run --task "" --repo "$PWD" --agent stub
rc=7
stdout_bytes=0 stderr_bytes=0
```

### The partial plan output is reproducible

```text
$ env -u FLEET_STATE ./keel/target/debug/fleet plan 'add a --version flag'
rc=3
stdout_bytes=55 stderr_bytes=264
stdout:
intent: implement a change
agent: builder
skills: rust
stderr begins:
fleet: environment fault: FLEET_STATE is not set.
```

The environment diagnostic is present, but it is emitted after the three plan lines, as claimed.

### The positive plan, refusal, SOW, and accepted-SOW paths

```text
$ env FLEET_STATE="$STATE" ./keel/target/debug/fleet plan 'add a --version flag to the cli'
rc=0, stdout_bytes=499, stderr_bytes=0

$ env FLEET_STATE="$STATE" ./keel/target/debug/fleet plan 'make me a sandwich'
rc=7, stdout_bytes=0, stderr_bytes=154

$ env FLEET_STATE="$STATE" ./keel/target/debug/fleet sow --task 'add a --version flag'
rc=7, stdout_bytes=0, stderr_bytes=806

$ env FLEET_STATE="$STATE" ./keel/target/debug/fleet sow --task "$TASK"
rc=9, stdout_bytes=1408, stderr_bytes=194
stderr: SOW_READY_AWAITING_REVIEW id=417272d39922cb374f423c2b2094ca2cd57d8086d321c14d06ead49ee1c1a444

$ env FLEET_STATE="$STATE" ./keel/target/debug/fleet sow accept --id 417272d39922cb374f423c2b2094ca2cd57d8086d321c14d06ead49ee1c1a444
rc=0, stdout_bytes=125, stderr_bytes=0
stdout: SOW_ACCEPTED id=... by=rachitsrivastava at=2026-08-26T13:33:54Z
```

The accepted-SOW run could not complete against this checkout:

```text
$ env FLEET_STATE="$STATE" ./keel/target/debug/fleet run --task "$TASK" --repo "$PWD" --agent stub
rc=7, stdout_bytes=0, stderr_bytes=519
stderr: fleet: refusing to run: the target repo has uncommitted changes.
```

Thus the claimed successful run was not replayable from the deliverable’s information. The
following state-dependent checks also demonstrate that the published `12/12` is not an inherent
result:

```text
$ env FLEET_STATE="$STATE" ./keel/target/debug/fleet status
rc=0, stdout_bytes=2482, stderr_bytes=0
stdout: FLEET STATUS — 1 of 1 tasks from receipts

$ env FLEET_STATE="$STATE" ./keel/target/debug/fleet ledger verify
rc=0, stdout_bytes=27, stderr_bytes=0
stdout: verified checked=5 total=5
```

### Tamper and restore

Using one fresh ledger row, changing its body, and restoring the saved row produced:

```text
$ env FLEET_STATE="$STATE" ./keel/target/debug/fleet ledger append --event note --body '{}'
rc=0, stdout_bytes=72, stderr_bytes=0

$ env FLEET_STATE="$STATE" ./keel/target/debug/fleet ledger verify   # after tamper
rc=8, stdout_bytes=0, stderr_bytes=0

$ env FLEET_STATE="$STATE" ./keel/target/debug/fleet ledger verify   # after restore
rc=0, stdout_bytes=27, stderr_bytes=0
stdout: verified checked=1 total=1
```

This confirms the silent mismatch defect. A fresh empty ledger is another silent non-zero path:

```text
$ env FLEET_STATE="$STATE" ./keel/target/debug/fleet ledger verify   # before append
rc=6, stdout_bytes=0, stderr_bytes=0
```

## Arithmetic and coverage findings

- The document reports two defects and then a third, but publishes no `checked/total` for the
  walkthrough, no number of refusal surfaces enumerated, and no list of omitted or untriggerable
  cases. `2 + 1 = 3` is a defect count, not a denominator.
- `checked=12 total=12` is only a historical success-path claim. The fresh replay produced `5/5`
  after the accepted-SOW attempt, and the isolated ledger replay produced `1/1`; no evidence ties
  `12/12` to the documented flow.
- `tests/corpus/M9.sh` is absent (`M9=absent`), and no dedicated walkthrough acceptance test is
  present (`walkthrough_acceptance=absent`).
- `FLEET_MUTANTS=0` skips mutation testing, so this run provides no mutation evidence.

## Required verifier result

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
process exit: 6
```

The shared `var/verify.log` contained 22 timeout failures and `M6: 23 of 24 documented commands
exist (denominator: 24)` at inspection. Multiple other verifier/reviewer processes were active in
the checkout, so those log details are not attributed to this one process; the terminal corpus
failure and exit 6 are authoritative for this run.

## Verdict: REJECT

The defects are real, but the deliverable is not acceptance evidence and does not satisfy the
nearest contract B8. The named contract is also missing, which prevents an unambiguous acceptance
decision.

For a resubmission, exactly these changes are required:

1. Add a standalone `opus-walkthrough` acceptance item, or explicitly bind this artifact to B8.
2. Rewrite the walkthrough as a self-contained transcript with the quoted checkout binary path,
   fresh-state setup, exact task/SOW text, repository and agent arguments, exit codes, stdout,
   stderr, and a checked/total denominator. Do not claim `12/12` without reproducing its state.
3. Fix the three reported user-surface defects: empty-task reason output, tampered-ledger reason
   output, and plan preflight/partial-output behavior. Include acceptance assertions.
4. Add `tests/corpus/M9.sh`, publish its full refusable-surface denominator, mutation-test both
   detection directions, and rerun `FLEET_MUTANTS=0 bash verify.sh` to a final exit-0 summary.
