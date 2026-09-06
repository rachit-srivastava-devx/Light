# Adversarial review: opus walkthrough

Review ID: `OPUS-WALKTHROUGH-ADVERSARIAL-20260825-CODEX-PRIMARY2`

Verdict: **REJECT**

## Contract under review

The walkthrough claims a successful end-to-end user flow and records three user-facing defects.
The applicable backlog contract is B8 in `handover/BACKLOG.md:92-106`: fix the three failures, add
`tests/corpus/M9.sh` with a published denominator, mutation-test it, and make `verify.sh` green.

## What I ran

All commands used the built binary at `keel/target/release/fleet`. Temporary state directories were
created with `mktemp -d`.

| Check | Exit | Observed |
|---|---:|---|
| `test -e tests/corpus/M9.sh` | 1 | M9 does not exist. `find tests/corpus -name '*.sh'` counted 105 files, including `run.sh` and `_selftest.sh`; the runner excludes those two. |
| `FLEET_STATE=<tmp> fleet run --task ""` (literal command shape) | 7 | `stdout=0`, `stderr=187`; this is an argument-usage refusal because required `--repo` is absent, not the claimed empty-task case. |
| `FLEET_STATE=<tmp> fleet run --task "" --repo "$PWD" --agent stub` | 7 | `stdout=0`, `stderr=0`; the valid empty-task refusal is silently reproduced. |
| `env -u FLEET_STATE fleet plan "add a --version flag to the cli"` | 3 | `stdout=55`, `stderr=264`; stdout contains `intent:`, `agent:`, and `skills:` before the environment-fault reason. |
| One-row ledger, clean `fleet ledger verify` | 0 | `verified checked=1 total=1` (`stdout=27`, `stderr=0`). |
| Same ledger after changing its row, `fleet ledger verify` | 8 | `stdout=0`, `stderr=0`; tampered-chain silence is reproduced. |
| Restored ledger, `fleet ledger verify` | 0 | `verified checked=1 total=1` (`stdout=27`, `stderr=0`). |
| Fresh-state `fleet ledger verify` | 6 | `stdout=0`, `stderr=0`; an additional silent non-zero exit not listed in the walkthrough. |
| Fresh-state `fleet status --json` | 0 | Returns `checked=0`, `total=0`, `empty=true`: a vacuous success that measures no tasks. |
| `fleet plan "add a --version flag to the cli"` | 0 | Plan printed, 499 stdout bytes. |
| `fleet plan "make me a sandwich"` | 7 | Refusal printed with three candidates, 154 stderr bytes. |
| `fleet run --task "add a --version flag" --repo "$PWD" --agent stub` | 7 | No-SOW refusal printed with an SOW id and recovery commands, 290 stderr bytes. |
| `fleet sow --task "add a --version flag"` | 7 | Missing-citation refusal and corrected-template guidance, 806 stderr bytes. |
| `FLEET_MUTANTS=0 bash verify.sh` | 6 | `14 passed, 1 failed, 1 skipped (denominator: 16 stages)`; the failed stage is `corpus`. |

Verifier output, including red:

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

The verifier log for that run reports:

```text
DENOMINATOR checked=34 total=34 excluded=69 caught=24
```

The corpus stage also reports detector timeouts as failures. This is a real red result, not an
interrupted or inferred result.

## Findings

1. **The two central defects remain.** The valid empty-task command still exits 7 with no reason,
   and a tampered ledger still exits 8 with no reason. These directly violate B8.

2. **The partial-output defect remains.** `plan_command` emits the three plan fields before calling
   `state_dir()` (`keel/fleet/src/main.rs:2659-2664`), so an unset `FLEET_STATE` produces a partial
   plan followed by exit 3.

3. **M9 is absent.** There is no `tests/corpus/M9.sh`, no M9 denominator, and no M9 mutation result.
   The existing corpus denominator (`34/34 checked`, `69 excluded`) is not evidence for the missing
   detector. The walkthrough's three-item count is therefore not a complete denominator for its
   stated property, “every non-zero exit.”

4. **The stated three failures are incomplete.** Fresh-state `ledger verify` is another silent
   non-zero exit (`rc=6`, 0 bytes on both streams). A detector covering “every non-zero exit” must
   include it or explicitly classify why it is excluded. The empty `status --json` result is also a
   zero-input vacuous success and needs an explicit contract decision; `checked=0,total=0` cannot be
   treated as evidence of working status reporting.

5. **The claimed `checked=12 total=12` success is not reproducible from the deliverable.** The doc
   gives no exact task text, state path, ledger rows, artifact id, or tamper operation. My controlled
   ledger had one row and correctly reported `1/1`; that validates the arithmetic of that run, not
   the undocumented twelve-row claim.

## Required changes before acceptance

1. Make the valid empty-task refusal, tampered-ledger mismatch, unset-state plan failure, and empty-
   ledger verification print a non-empty line naming the reason on every non-zero path.
2. Add `tests/corpus/M9.sh` with an enumerated surface denominator. It must exercise both successful
   and refusal directions, reject zero-input/vacuous coverage, and assert the reason text rather than
   only exit codes.
3. Mutation-test M9: show the detector red when its reason assertion is removed or bypassed, then
   restore it and record the caught/total result.
4. Resolve the current corpus failures and rerun `FLEET_MUTANTS=0 bash verify.sh`; acceptance needs
   exit 0 with the final `passed/failed/skipped` denominator pasted.
5. Replace the walkthrough's uncheckable twelve-row statement with exact reproducible commands and
   published counts for the clean, tampered, and restored ledger states.
