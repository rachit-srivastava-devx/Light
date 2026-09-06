# Adversarial review: opus walkthrough

Date: 2026-08-25

Deliverable: `docs/delta.d/opus-walkthrough.md`

Contract: B8, “Every non-zero exit must print a reason,” in `handover/BACKLOG.md`.

## Verdict

**REJECT**

The three defects are still reproducible, `tests/corpus/M9.sh` is absent, and the required
verifier is red. The backlog item remains unchecked.

## What was claimed

The walkthrough records three user-facing defects:

1. Empty `run` exits 7 with no stdout or stderr.
2. A tampered ledger exits 8 with no output.
3. `plan` emits `intent:`, `agent:`, and `skills:` before an environment failure when
   `FLEET_STATE` is unset.

It proposes a detector requiring every non-zero exit to print a reason. It does not claim that B8
has been fixed.

## What I ran

All commands used the built user binary at `keel/target/release/fleet`; no `git` command was run.

### Empty task

Literal walkthrough shorthand, with a fresh state directory:

```text
FLEET_STATE=<fresh dir> fleet run --task ""
exit=7 stdout_bytes=0 stderr_bytes=187
stderr: fleet: run: --repo is required. ...
```

Finding: the shorthand in the walkthrough is not a self-contained reproduction; missing required
arguments take a different, explained refusal path. With the complete user invocation needed to
reach the empty-task branch:

```text
FLEET_STATE=<fresh dir> fleet run --task "" --repo "$PWD" --agent stub
exit=7 stdout_bytes=0 stderr_bytes=0
```

This confirms the underlying silent failure, but the walkthrough should publish the exact complete
command and prerequisites.

### Tampered ledger

I appended a real ledger row, changed its body from `body-before` to `body-after`, and verified it:

```text
ledger append --event note --body '{"review":"body-before"}'  exit=0
fleet ledger verify                                             exit=8
stdout_bytes=0 stderr_bytes=0
```

The failure is detected, but the operator receives no reason. This matches the walkthrough’s
second finding.

### Plan with no state

```text
env -u FLEET_STATE fleet plan "make a small change"  exit=3
stdout:
intent: implement a change
agent: builder
skills: rust
stderr:
fleet: environment fault: FLEET_STATE is not set.
```

This matches the third finding: the command exposes a partial plan before refusing.

### Missing detector

```text
bash tests/corpus/M9.sh
exit=127
bash: tests/corpus/M9.sh: No such file or directory
```

`tests/corpus/M9.sh` does not exist. The acceptance criterion explicitly requires it, a published
denominator, and mutation testing.

## Independent verifier

Exact required command:

```text
FLEET_MUTANTS=0 bash verify.sh
```

Exit code: **6**.

Real final output:

```text
  FAIL corpus                     (see var/verify.log)
-- 14 passed, 1 failed, 1 skipped (denominator: 16 stages) --
```

The skipped stage was mutants, explicitly skipped because `FLEET_MUTANTS=0`. The corpus log
reported:

```text
detector-integrity: 105 detectors match the manifest (denominator: 105)
  TIMEOUT A1.sh exceeded 30s -- treated as FAILED
  TIMEOUT A3.sh exceeded 30s -- treated as FAILED
  TIMEOUT B10.sh exceeded 30s -- treated as FAILED
M2: 33944 files in the tree (>15000). Build artifacts are almost certainly inside the repo;
    set CARGO_TARGET_DIR outside it. Detectors walk the tree and will stall (D30).
DENOMINATOR checked=34 total=34 excluded=69 caught=21
```

Arithmetic: `14 + 1 + 1 = 16` stages. For the corpus, `34 checked + 69 excluded = 103` runnable
scripts; `caught=21` means the checked detector set was not an all-green result. The denominator is
published, but it does not include the absent M9 detector.

## Required changes for acceptance

1. Make the empty-task and tampered-ledger paths print a non-empty, actionable reason before exit.
2. Validate `FLEET_STATE` before rendering the plan, or otherwise prevent partial output before an
   environment refusal.
3. Add `tests/corpus/M9.sh`. It must drive every refusable surface, assert a reason for every
   non-zero exit, publish its checked/total denominator, and fail when a surface cannot be
   triggered rather than silently excluding it.
4. Mutation-test M9 in both directions, then rerun the exact verifier and capture an exit-0 final
   summary with the M9 denominator included.
