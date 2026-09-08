# Adversarial review: opus walkthrough

Deliverable: `docs/delta.d/opus-walkthrough.md`  
Contract: `handover/BACKLOG.md` B8 (“Every non-zero exit must print a reason”)

## Verdict: REJECT

The underlying silent-failure observations are real when invoked with the missing
preconditions supplied, but the deliverable is not a reproducible walkthrough and the
B8 acceptance contract is not met. The three defects remain, `M9.sh` is absent, and the
required verifier is red.

## What was claimed

The document claims that a full user flow works end to end, then reports three defects:

1. `fleet run --task ""` exits 7 with zero stdout/stderr.
2. Tampered `fleet ledger verify` exits 8 without output.
3. A valid `fleet plan` with `FLEET_STATE` unset prints `intent:`, `agent:`, and `skills:`
   before the environment fault.

It also claims the code is right and only the user-facing layer is uncovered, and proposes
a detector for every non-zero exit.

## Commands actually run

All manual CLI cases used `keel/target/release/fleet`; no direct `git` command was run.

| Case | Exit | stdout | stderr | Result |
|---|---:|---:|---:|---|
| Literal `fleet run --task ""`, no state | 3 | 0 B | 64 B | Environment error, not claimed exit 7 |
| `FLEET_STATE=<tmp> fleet run --task ""` | 7 | 0 B | 187 B | Usage error: `--repo` is required |
| `FLEET_STATE=<tmp> fleet run --task "" --repo "$PWD" --agent stub` | 7 | 0 B | 0 B | Silent defect reproduced |
| Tampered chain: `fleet ledger verify` | 8 | 0 B | 0 B | Silent defect reproduced |
| Restored chain: `fleet ledger verify` | 0 | 27 B | 0 B | `verified checked=1 total=1` |
| Literal `env -u FLEET_STATE fleet plan` | 7 | 0 B | 41 B | Usage error, no partial plan |
| `env -u FLEET_STATE fleet plan "add a --version flag"` | 3 | 55 B | 264 B | Partial-output defect reproduced |

The tamper setup was a real ledger: `ledger append --event note --body '{"review":1}'`
returned exit 0, and the restored verification published `checked=1 total=1`.

## Findings

### F1 — The cited commands are under-specified and do not reproduce the claims

The literal `fleet run --task ""` does not yield the documented result. The silent result
requires `FLEET_STATE`, `--repo`, and `--agent`; without them the command emits a reason or
an environment fault. The literal bare `fleet plan` also does not reach the partial-output
path; it needs a valid classified prompt. The document gives no exact state setup, repo
fixture, tamper operation, command output, or exit transcript for the claimed “full flow”.

This is an evidence failure, not merely a wording preference: a reviewer cannot reproduce
the reported cases from the document as written.

### F2 — B8 is not implemented

The current binary still has the defects:

- `run_with_evidence` records `EMPTY_TASK` and returns exit 7 without printing a reason
  (`keel/fleet/src/main.rs:903-912`).
- `ledger_verify` prints only after `verify_rows` succeeds; verification mismatch returns
  before any diagnostic (`keel/fleet/src/main.rs:2995-3000`).
- `plan_command` prints the first three plan lines before calling `state_dir()`
  (`keel/fleet/src/main.rs:2659-2664`).

`tests/corpus/M9.sh` does not exist (direct `test -f` exit 1), so there is no published M9
denominator and no mutation evidence for the required all-refusable-surfaces check.

### F3 — The full verifier is red, with a non-vacuous corpus denominator

Exact required command:

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

The corpus log published its own denominator: `checked=34 total=34 excluded=69 caught=23`.
That arithmetic is consistent (`34 + 69 = 103` detector files; `23` caught and `11`
non-caught among checked), but it is red. A green targeted check cannot earn B8.

## Required changes for acceptance

1. Fix all three user-visible paths so every non-zero result emits a line naming the reason;
   preserve the refusal receipt and typed exit code.
2. Rewrite the walkthrough with exact commands, temporary state/repo prerequisites, tamper
   setup, exit codes, byte counts, and a denominator for every claimed exercise. Do not use
   an unqualified command as proof of a state-dependent result.
3. Add `tests/corpus/M9.sh` covering every reachable refusable surface, publish its checked/
   total denominator, and ensure empty input is not a vacuous pass.
4. Mutation-test M9 in both directions, update detector integrity, and rerun the full verifier
   until the final result is exit 0 with its stage denominator recorded.
