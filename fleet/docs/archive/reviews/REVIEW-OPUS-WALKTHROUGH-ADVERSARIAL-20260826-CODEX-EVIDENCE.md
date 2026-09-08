# Adversarial review: opus walkthrough

Date: 2026-08-26

## Verdict

REJECT

The three silent paths are real, but the deliverable is not acceptance evidence for B8. The
successful end-to-end flow was not reproducible in this checkout, no denominator is published,
`M9.sh` is absent, and the required verifier is red.

## Contract used

There is no standalone `opus-walkthrough` heading in `handover/BACKLOG.md`; the only binding
reference is B8 at lines 92–106. B8 requires all three paths fixed, `tests/corpus/M9.sh` with a
published denominator, mutation testing, and a green `verify.sh`.

## What was claimed

`docs/delta.d/opus-walkthrough.md` claims:

- a complete plan/refusal/SOW/accepted-SOW/run/status/ledger/tamper/restore flow works end to end;
- `fleet run --task ""` exits 7 with no output;
- tampered `fleet ledger verify` exits 8 with no output;
- an unset-state `fleet plan` emits three lines before its environment fault;
- a detector asserting that every non-zero exit names its reason would catch all three.

The document reports three observations but publishes no `checked/total` denominator, no complete
surface inventory, no mutation result, and no exact command transcript for the claimed successful
flow.

## Commands actually run

The repo-local binary was built with:

```text
cargo build --manifest-path keel/Cargo.toml
exit 0
```

Using a disposable state and target repository, the named walkthrough surfaces produced:

| command | exit | stdout bytes | stderr bytes | observed |
|---|---:|---:|---:|---|
| `fleet plan "add a --version flag to the cli"` with state | 0 | 499 | 0 | plan printed, route unavailable, denominator 3 |
| `fleet run --task "" --repo <repo> --agent stub` | 7 | 0 | 0 | silent empty-task refusal |
| `fleet run --task ""` shorthand | 7 | 0 | 187 | argument error was visible; this is not the deliverable's primary reproduction |
| `fleet run --task "add a --version flag" --repo <repo> --agent stub` | 7 | 0 | 290 | visible `SOW_NOT_ACCEPTED` pushback |
| `fleet sow --task "add a --version flag"` | 7 | 0 | 806 | visible missing-citation template |
| valid `fleet sow --task <multiline SOW>` | 9 | 1480 | 194 | SOW was created and awaiting review |
| `fleet sow accept --id <id>` | 0 | 125 | 0 | SOW accepted |
| `fleet run --task <same SOW> --repo <repo> --agent stub` | 6 | 0 | 204 | stub exited without an fd-3 result; no artifact was produced |
| `fleet status` after that run | 0 | 2656 | 0 | task remained `PENDING`; no completed task |
| `fleet ledger verify` before tamper | 0 | 27 | 0 | `verified checked=5 total=5` |
| tampered `fleet ledger verify` | 8 | 0 | 0 | silent mismatch |
| restored `fleet ledger verify` | 0 | 27 | 0 | `verified checked=5 total=5` |
| `fleet plan "add a --version flag to the cli"` with `FLEET_STATE` unset | 3 | 55 | 264 | stdout contained `intent:`, `agent:`, `skills:` before the environment fault |

The later valid-SOW state contained `verified checked=8 total=8`, but that was after the failed
stub run and was not a successful end-to-end artifact flow. Trying `--agent env-probe` also exited
6 with `fleet: refusing to freeze: the agent landed no NEW work.`

## Coverage and arithmetic

Directly running the required detector path:

```text
$ bash tests/corpus/M9.sh
bash: tests/corpus/M9.sh: No such file or directory
$ echo $?
127
```

The checkout has 105 `tests/corpus/*.sh` files, of which `run.sh` and `_selftest.sh` are helpers;
the runner's non-helper denominator is therefore 103. The manifest also has 105 rows. Neither
number is an M9 denominator, and there is no checked surface count for the proposed “every
refusable surface” claim. An absent detector cannot be mutation-tested or pass vacuously.

## Required verifier

Exact command run:

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

The shared `var/verify.log` was concurrently touched by other workers, so this review relies on
the command's own final stdout and exit code, not on a potentially mixed log.

## Findings

1. **The successful end-to-end claim is not reproducible.** The accepted SOW run with the
   documented deterministic agent exited 6 before producing an fd-3 result; status showed a
   pending task and no artifact. The deliverable has no state setup, artifact id, output, or exit
   transcript that would make its historical success independently checkable.

2. **The primary empty-task defect is real, but the document conflates it with argument
   validation.** The complete invocation with `--repo` and `--agent` is silent; the shorthand
   invocation is not silent because it fails earlier on missing `--repo`. The distinction must be
   explicit in any reproduction.

3. **The walkthrough has no denominator.** Three findings are not evidence for “every” refusable
   surface. Empty-store verification, missing arguments, invalid arguments, environment faults,
   target-repository faults, mismatch paths, and other command dispatch refusals are not classified.

4. **B8 is not met.** `M9.sh` is missing, no mutation result exists, and the exact required gate
   exits 6 with corpus red. The backlog item remains unchecked.

5. **The implementation still has the claimed output-order defects.** Empty-task validation
   records `EMPTY_TASK` and returns exit 7 without printing a reason; `ledger_verify` prints only
   after `verify_rows` succeeds, so mismatch errors return before any message; `plan_command`
   prints intent/agent/skills before `state_dir()` can return the environment fault.

## Exactly what must change

1. Fix those three paths so every non-zero result writes at least one human-readable line naming
   the reason, while preserving the typed exit codes and refusal receipts.
2. Correct the walkthrough to show the complete empty-task command, disposable state setup, exact
   stdout/stderr and exit codes, and a genuinely successful artifact flow—or label the flow
   unverified when the agent cannot complete it.
3. Add `tests/corpus/M9.sh` covering every reachable refusable surface, fail if zero surfaces are
   checked, publish `checked/total`, classify omitted/untriggerable surfaces, and ensure its scans
   do not fire on their own documentation.
4. Mutation-test M9 in both directions: removing a reason must make it red, and an empty or
   untriggerable surface set must not pass. Update the detector manifest deliberately.
5. Rerun `FLEET_MUTANTS=0 bash verify.sh` to a final exit 0 with the full 16-stage denominator;
   only then can B8 be marked complete.
