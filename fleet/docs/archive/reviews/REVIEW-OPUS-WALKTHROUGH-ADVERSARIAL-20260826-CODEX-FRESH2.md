# Adversarial review: opus walkthrough

Verdict: **REJECT**

The walkthrough correctly identified real silent failure paths, but it is not an acceptance of
B8. The three defects are still present, M9 does not exist, mutation evidence is absent, and the
required verifier is red.

## What was claimed

`docs/delta.d/opus-walkthrough.md` claims that the plan/SOW/run/status/ledger/tamper flow worked
end to end, and reports three user-facing defects:

1. Empty `run` exits 7 with no output.
2. Tampered `ledger verify` exits 8 with no output.
3. A plan with `FLEET_STATE` unset prints partial output before its environment refusal.

The B8 contract in `handover/BACKLOG.md` requires all three fixed, `tests/corpus/M9.sh` with a
published denominator and mutation test, and a green `verify.sh`.

## What I ran

All binary runs used the existing release binary at
`keel/target/release/fleet` and isolated directories created with `mktemp -d`. No Git command was
run.

| Scenario | Exit | stdout / stderr | Observation |
|---|---:|---:|---|
| `FLEET_STATE=<tmp> fleet run --task ""` | 7 | 0 / 187 bytes | The literal shorthand refuses earlier because required `--repo` is missing; it does not reproduce the claimed silent path. |
| `FLEET_STATE=<tmp> fleet run --task "" --repo "$PWD" --agent stub` | 7 | **0 / 0 bytes** | Silent empty-task refusal reproduced. The receipt records `EMPTY_TASK`, but the user receives no reason. |
| `env -u FLEET_STATE fleet plan "add a --version flag to the cli"` | 3 | 55 / 264 bytes | `intent:`, `agent:`, and `skills:` are printed before the environment fault. |
| `FLEET_STATE=<fresh> fleet status --json` | 0 | 452 / 0 bytes | Returns `checked=0`, `total=0`, `empty=true`: a vacuous success on zero inputs. |
| `FLEET_STATE=<fresh> fleet ledger verify` | 6 | **0 / 0 bytes** | Additional silent non-zero path omitted by the walkthrough. |
| valid copied 9-row chain, `fleet ledger verify` | 0 | 27 / 0 bytes | `verified checked=9 total=9`. |
| same chain after changing one body field, `fleet ledger verify` | 8 | **0 / 0 bytes** | Silent tamper mismatch reproduced. |

The broader user flow was also driven with a complete multiline SOW: plan `0`, nearest-intent
plan `7`, run before SOW `7`, incomplete SOW `7`, complete SOW `9`, SOW accept `0`, status `0`,
and ledger verify `0` with `checked=5 total=5`. The post-acceptance run did **not** work in this
checkout: it exited `7` with 519 bytes because the target repository had uncommitted changes.
Therefore the deliverable's unqualified “works end to end” claim is not independently reproducible
here; it needs the exact clean-repository precondition and a complete transcript.

The required detector is absent:

```text
$ bash tests/corpus/M9.sh
bash: tests/corpus/M9.sh: No such file or directory
exit=127
```

No M9 denominator or M9 mutation result exists. The walkthrough itself publishes no denominator
for its full flow or for the proposed “every non-zero exit” coverage. Its `checked=12 total=12`
example is only a ledger-row denominator, not coverage of refusable surfaces.

## Independent verifier

Command: `FLEET_MUTANTS=0 bash verify.sh`

Real result, including red output:

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

The corpus log published:

```text
M6: 22 of 23 documented commands exist (denominator: 23)
DENOMINATOR checked=34 total=34 excluded=69 caught=17
```

It also recorded timeout failures for A1, A3, A4, A8, C11, C2, C22, C26, C6, S6, S9, T1,
T15, and T5. The verifier skipped mutation testing by design because the requested command sets
`FLEET_MUTANTS=0`; therefore it provides no evidence for the B8 mutation requirement.

## Findings

### 1. B8's three defects remain live

Failure -> Cause -> Fix:

- Empty task -> `run_with_evidence` appends an `EMPTY_TASK` receipt and returns exit 7 without
  printing -> emit a diagnostic naming `EMPTY_TASK` before returning.
- Tampered ledger -> `ledger_verify` propagates `verify_rows`' mismatch before its success-only
  `println!` -> catch/report the failing reason and checked position on stderr, while preserving
  exit 8.
- Unset state during plan -> `plan_command` prints intent/agent/skills before calling the state
  lookup -> validate `FLEET_STATE` first or buffer all plan output until validation succeeds.

### 2. Required M9 is missing

The required file is absent, so no detector drives the refusable surfaces, names reasons, publishes
the denominator, or proves both refusal and non-refusal directions. A green existing suite cannot
substitute for this missing contract-specific detector.

### 3. End-to-end evidence is incomplete and the denominator is missing

The deliverable has no exact task text, state setup, binary path, clean target-repository
precondition, exit-code transcript, artifact ID, or count of attempted/excluded cases. The only
ratio shown, `12/12`, is not a walkthrough denominator. The successful run claim cannot be replayed
from this file and did not complete in the current checkout.

### 4. Additional vacuous success is omitted

Fresh `status --json` exits 0 while checking zero inputs. That violates the repository rule that a
check examining zero inputs must fail or explicitly report an unmeasured state, and it weakens the
walkthrough's claim that the user-facing flow is covered.

## Exactly what must change before acceptance

1. Implement and manually re-run the three fixes above, recording exit code plus stdout/stderr
   bytes and a human-readable reason for each non-zero result.
2. Add `tests/corpus/M9.sh`. It must exercise every refusable surface it claims to cover, fail on
   an untriggerable surface, avoid matching its own documentation, test both refusal and success
   directions, and print `checked`, `total`, and any excluded/untriggerable count.
3. Mutation-test M9 by weakening/removing each relevant guard, show the detector goes red, restore
   the guard, and record the actual mutation result. Do not treat the `FLEET_MUTANTS=0` verifier
   run as mutation evidence.
4. Make `FLEET_MUTANTS=0 bash verify.sh` finish green with its final exit code and denominator,
   including no corpus red stages or silent timeout substitutions.
5. Replace the end-to-end prose with a replayable transcript from an isolated state and clean
   target repository, including the artifact ID and the published walkthrough denominator.

