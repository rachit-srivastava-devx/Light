# Adversarial review: opus-walkthrough

## Verdict

**REJECT**

I treated B8 in `handover/BACKLOG.md` (lines 92-106) as the acceptance contract because it is the
backlog item that names `docs/delta.d/opus-walkthrough.md` and supplies the required criteria.
The contract requires all three paths to be fixed, `tests/corpus/M9.sh` with a published denominator,
mutation evidence, and a green `verify.sh`.

## What was claimed

`docs/delta.d/opus-walkthrough.md` claims that a manual end-to-end flow found:

1. Empty-task `run` exits 7 with no stdout or stderr.
2. Tampered-ledger `verify` exits 8 with no stdout or stderr.
3. `plan` with `FLEET_STATE` unset emits three plan lines before the environment fault.

It proposes a detector asserting that every non-zero exit writes a reason. It does not claim that the
B8 fixes, M9, or mutation testing are already implemented, but those are the acceptance requirements
for this deliverable's referenced backlog item.

## Commands actually run

The installed `fleet` command was unavailable:

```text
env FLEET_STATE=<mktemp> fleet run --task ""
exit=127
stderr: env: fleet: No such file or directory
```

I then ran the built release binary at `./keel/target/release/fleet` as the local-user equivalent.
All state directories were disposable `mktemp -d` paths.

### Empty task

The literal command shape without `--repo` does not reach empty-task validation:

```text
FLEET_STATE=<state> ./keel/target/release/fleet run --task ""
exit=7, stdout=0 bytes, stderr=187 bytes
stderr: fleet: run: --repo is required.
```

With the required repository and agent arguments, the reported failure reproduces exactly:

```text
FLEET_STATE=<state> ./keel/target/release/fleet run --task "" --repo . --agent stub
exit=7, stdout=0 bytes, stderr=0 bytes
```

### Plan with missing state

```text
env -u FLEET_STATE ./keel/target/release/fleet plan "add a --version flag"
exit=3, stdout=55 bytes, stderr=264 bytes
stdout:
intent: implement a change
agent: builder
skills: rust
stderr begins:
fleet: environment fault: FLEET_STATE is not set.
```

The partial-output claim is reproduced. The command prints user-facing plan content before refusing.

### Ledger success and tamper failure

I appended 12 ledger rows and verified them:

```text
FLEET_STATE=<state> ./keel/target/release/fleet ledger verify
exit=0
verified checked=12 total=12
```

I then changed one stored body field in the chain and ran the same verification:

```text
FLEET_STATE=<state> ./keel/target/release/fleet ledger verify
exit=8, stdout=0 bytes, stderr=0 bytes
```

The tampered-chain claim is reproduced. The success denominator is internally consistent for 12 rows,
but it says nothing about the failure-path diagnostic.

### Required M9 check

```text
test -e tests/corpus/M9.sh
exit=1

bash tests/corpus/M9.sh
exit=127
stderr: bash: tests/corpus/M9.sh: No such file or directory
```

No M9 reference exists in `tests/corpus`, `bin`, `verify.sh`, or `keel/mutants.out` in the checked
tree. Therefore there is no M9 checked/total denominator and no M9 mutation result.

### Independent full verifier

Exact command required by the handover:

```text
FLEET_MUTANTS=0 bash verify.sh
```

Observed result:

```text
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

`var/verify.log` reports:

```text
TIMEOUT A1.sh ...
TIMEOUT A3.sh ...
TIMEOUT A4.sh ...
TIMEOUT A8.sh ...
TIMEOUT B10.sh ...
TIMEOUT C1.sh ...
TIMEOUT C11.sh ...
TIMEOUT C12.sh ...
TIMEOUT C19.sh ...
TIMEOUT C2.sh ...
TIMEOUT C22.sh ...
TIMEOUT C24.sh ...
TIMEOUT C26.sh ...
TIMEOUT C3.sh ...
TIMEOUT C6.sh ...
TIMEOUT C8.sh ...
TIMEOUT C9.sh ...
TIMEOUT S4.sh ...
TIMEOUT S6.sh ...
TIMEOUT S9.sh ...
TIMEOUT T1.sh ...
TIMEOUT T15.sh ...
TIMEOUT T20.sh ...
TIMEOUT T5.sh ...
TIMEOUT T6.sh ...
DENOMINATOR checked=34 total=34 excluded=69 caught=27
```

The stage arithmetic is 14 + 1 + 1 = 16. The existing generic corpus runner evaluated 34 cases,
excluded 69, and classified 27 as caught failures; it is not an M9 denominator and cannot substitute
for M9's required refusable-surface coverage. `FLEET_MUTANTS=0` deliberately skips mutation testing,
so this verifier run is not mutation evidence.

## Findings

### F1 — REJECT: two user-facing failure paths are still silent

The valid-repository empty-task command still exits 7 with zero bytes, and a tampered ledger still
exits 8 with zero bytes. These are the exact defects the backlog says must be fixed.

### F2 — REJECT: `plan` still emits partial output before its environment refusal

With `FLEET_STATE` unset, `plan` prints `intent`, `agent`, and `skills` before exit 3. A user sees a
partial plan followed by a refusal. Validate required environment before emitting the plan, or emit a
single complete diagnostic without partial plan content.

### F3 — REJECT: required M9 detector and denominator are absent

`tests/corpus/M9.sh` does not exist and its required invocation exits 127. The existing corpus count
does not enumerate every refusable surface and does not prove the reason-output property. The detector
must fail on zero checked inputs, classify hard-to-trigger surfaces explicitly, and publish
`checked/total` plus exclusions.

### F4 — REJECT: mutation testing and green-gate evidence are absent

No M9 mutation result exists. The required verifier is red with exit 6, and its mutants stage was
skipped by the mandated `FLEET_MUTANTS=0` setting. A generic or skipped mutation stage cannot satisfy
the B8 requirement that M9 itself be mutation-tested.

## Exactly what must change for acceptance

1. Make the empty-task refusal and tampered-ledger mismatch print a non-empty, reason-naming
   diagnostic while preserving exit codes 7 and 8 and refusal receipts.
2. Fix `plan` so an environment refusal does not emit partial plan output; its non-zero exit must also
   name the reason.
3. Add `tests/corpus/M9.sh` that executes every reachable refusable surface, asserts non-zero exit
   plus a reason line, refuses a zero-input run, excludes documentation-only matches, and publishes a
   non-zero `checked/total` denominator with exclusions and classifications.
4. Mutation-test M9: remove or bypass its reason assertion and show M9 red; restore it and show M9
   green. Record the actual caught/total result and update detector integrity if required.
5. Rerun `FLEET_MUTANTS=0 bash verify.sh` to a final exit 0, with the final stage denominator and all
   red output resolved.
