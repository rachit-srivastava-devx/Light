# Adversarial review: opus walkthrough

Reviewed `docs/delta.d/opus-walkthrough.md` on 2026-08-25.

## Contract

There is no BACKLOG item literally named `opus-walkthrough`. The matching contract is the unchecked
B8 item, `Every non-zero exit must print a reason`, at `handover/BACKLOG.md:92-106`:

- fix the three reported paths;
- add `tests/corpus/M9.sh` with a published denominator and mutation evidence;
- finish with `verify.sh` green.

## What was claimed

The deliverable claims that a complete plan/refusal/SOW/accept/run/status/ledger/tamper/restore flow
worked, that the SOW refusal was actionable, and that these three defects were observed:

1. empty-task `run` exits 7 with zero output;
2. tampered `ledger verify` exits 8 with zero output;
3. task-bearing `plan` with `FLEET_STATE` unset emits `intent`, `agent`, and `skills` before exit 3.

It proposes a detector requiring a reason on every non-zero exit, but does not provide a command
transcript, stream byte counts for the full flow, a tested-surface denominator, or mutation evidence.

## Commands actually run

All Fleet commands used the current release binary `keel/target/release/fleet`, with quoted paths and
fresh `mktemp -d` state directories. Exit codes and stdout/stderr byte counts were captured before
cleanup.

| Case | Exit | stdout | stderr | Observation |
|---|---:|---:|---:|---|
| `fleet run --task ""` exactly as written | 7 | 0 | 187 | Does not reach empty-task validation; it visibly refuses because `--repo` is required. |
| `fleet run --task "" --repo "$PWD" --agent stub` | 7 | 0 | 0 | Valid-argument empty-task refusal is silent, reproducing the underlying defect. |
| `env -u FLEET_STATE fleet plan "add a --version flag"` | 3 | 55 | 264 | stdout contains `intent:`, `agent:`, `skills:` before the environment diagnostic. |
| 12 ledger appends, then clean `fleet ledger verify` | 0 | 29 | 0 | `verified checked=12 total=12`. The denominator adds up. |
| Same 12-row chain after changing one body field | 8 | 0 | 0 | Silent tamper mismatch, reproducing the claim. |
| Restored 12-row chain, then `fleet ledger verify` | 0 | 29 | 0 | Returns to `verified checked=12 total=12`. |
| `fleet status` on the isolated state | 0 | 2483 | 0 | Renders `empty store; 0 of 0 tasks`; this is a separate vacuous-success risk. |

The other walkthrough pushbacks were also exercised: normal `plan` exited 0 with a 3-command plan;
the sandwich plan exited 7 with three candidates; run-before-SOW exited 7 with a SOW id; incomplete
SOW exited 7 with a corrected template; filled SOW exited 9; and `sow accept` exited 0. The claimed
successful run could not be reproduced in this checkout: after acceptance, `fleet run ... --repo
"$PWD" --agent stub` exited 7 because the shared target repository had uncommitted changes. No
artifact or post-run status was therefore produced by this review.

## Acceptance artifact checks

```text
M9_EXISTS=0
```

`tests/corpus/M9.sh` is absent. No M9 checked/total denominator or M9 mutation result exists. The
existing corpus contains 105 shell files including its runner, but that is not an M9 denominator and
does not test the new non-zero-output property.

## Independent verifier

Command:

```text
FLEET_MUTANTS=0 bash verify.sh
```

Real terminal result:

```text
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
```

The verifier exits 6 from its failed-stage branch. `var/verify.log` reports corpus timeouts treated
as failures and ends with:

```text
DENOMINATOR checked=34 total=34 excluded=69 caught=17
```

That is red evidence, not acceptance. Mutation testing was skipped by the requested `FLEET_MUTANTS=0`
mode, and there is no M9 to mutation-test separately.

## Verdict: REJECT

The two silent runtime defects are real. The partial-plan defect is real. The literal empty-task
command in the deliverable is incomplete, so it is not itself a valid reproduction. The full
successful run claim is not independently reproducible from the current checkout, and the B8
acceptance contract is plainly unmet: M9 is absent and the required verifier is red.

## Exactly what must change

1. Correct the walkthrough to show the complete empty-task invocation and publish exact stdout,
   stderr, exit code, binary path, state setup, and repository precondition for every command.
2. Fix the valid empty-task refusal, tampered-ledger mismatch, and missing-state plan path so every
   non-zero exit emits at least one human-readable reason. Decide and cover the empty-ledger exit-6
   path too; it is also silent.
3. Add `tests/corpus/M9.sh` covering every refusable surface, including the three reported cases and
   the empty-ledger case. Publish `checked/total`; fail when checked is zero; include surfaces that
   cannot be triggered as failures, not skipped passes.
4. Mutation-test M9 in both directions, update detector integrity as required by the repository law,
   and record the real mutation result and denominator.
5. Rerun `FLEET_MUTANTS=0 bash verify.sh` to exit 0 with no failed stage. Do not call the B8 item
   accepted while the corpus stage is red or while M9 and its mutation evidence are missing.
6. Either reproduce the successful accepted-SOW run on a clean disposable target and include its
   actual `run`/`status`/ledger/tamper/restore evidence, or downgrade the walkthrough’s end-to-end
   claim to unverified.
