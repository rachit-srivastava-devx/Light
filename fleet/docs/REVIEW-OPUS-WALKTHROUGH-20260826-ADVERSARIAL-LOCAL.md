# Adversarial review: opus-walkthrough

## Verdict

**REJECT**

The walkthrough records real failure behavior, but it does not satisfy its contract. The three
reported defects remain, tests/corpus/M9.sh is absent, no M9 denominator or mutation evidence is
published, and the required verifier did not complete green.

The acceptance contract is B8 in handover/BACKLOG.md: fix the three paths, add a detector that
publishes a denominator and is mutation-tested, and make verify.sh green.

## What was claimed

docs/delta.d/opus-walkthrough.md claims that:

1. A complete plan -> refusal -> run -> SOW refusal -> SOW acceptance -> run -> status -> ledger
   verify -> tamper -> restore flow works end to end.
2. fleet run --task "" exits 7 with zero bytes on stdout and stderr.
3. Tampered fleet ledger verify exits 8 with no output, while the clean path prints
   verified checked=12 total=12.
4. Unset FLEET_STATE makes fleet plan print intent:, agent:, and skills: before the environment
   fault.
5. A detector should require every non-zero exit to emit a reason.

## What I actually ran

I did not run Git. fleet is not on PATH (command -v fleet exited 1), so I used the checked-in
release binary at keel/target/release/fleet and isolated state directories made with mktemp -d.

| Command / case | Exit | stdout | stderr | Observation |
|---|---:|---:|---:|---|
| keel/target/release/fleet --version | 0 | 0 B | 0 B | Binary exists; shell command fleet itself is not installed on PATH. |
| Exact cited fleet run --task "" | 7 | 0 B | 187 B | Does not reproduce the claimed empty-task case; it stops earlier with --repo is required. |
| FLEET_STATE=<tmp> fleet run --task "" --repo "$PWD" --agent stub | 7 | 0 B | 0 B | The valid-argument empty-task refusal is silent. Reproduced. |
| env -u FLEET_STATE fleet plan "add a --version flag" | 3 | 55 B | 264 B | Prints the three plan fields before the environment-fault message. Reproduced. |
| FLEET_STATE=<tmp> fleet ledger verify on a fresh state | 6 | 0 B | 0 B | Silent non-zero verification path. Reproduced. |
| FLEET_STATE=<tmp> fleet attest verify <64 zeroes> | 8 | 0 B | 0 B | Another silent non-zero path omitted by the walkthrough. |
| fleet sow --task "add a --version flag" | 7 | 0 B | 806 B | SOW refusal names the missing citation and prints a recovery template. |
| Complete README-shaped SOW | 9 | 1408 B | 194 B | SOW emitted with a 64-character id and SOW_READY_AWAITING_REVIEW. |
| fleet sow accept --id <that id> | 0 | 125 B | 0 B | Acceptance receipt emitted. |
| Accepted fleet run against this shared checkout | 7 | 0 B | 519 B | Refused because the checkout has uncommitted changes; no artifact or successful run was observed. |
| fleet status after that flow | 0 | 2483 B | 0 B | Reports empty store; 0 of 0 tasks despite four ledger receipts. |
| fleet ledger verify after that flow | 0 | 27 B | 0 B | Reports verified checked=4 total=4. |

For the tamper control, I appended exactly 12 ledger rows to a fresh state, changed one hash field,
then restored the original file:

~~~text
append_1..append_12: all exit 0
clean:    rc=0 stdout=29 B stderr=0 B  verified checked=12 total=12
tampered: rc=8 stdout=0 B  stderr=0 B
restored: rc=0 stdout=29 B stderr=0 B  verified checked=12 total=12
~~~

This clean 12/12 is a separate control state. In the attempted walkthrough sequence, an empty-task
refusal itself appended a receipt, so a later 12-row append verified as checked=13 total=13. The
walkthrough publishes no state directory, task text, repository fixture, SOW id, tamper edit, or
per-command transcript from which its 12/12 can be recomputed.

The fresh-store status check also exposes an adjacent vacuous result:

~~~text
FLEET_STATE=<fresh> fleet status --json
exit 0
{ "checked": 0, "total": 0, "empty": true, ... }
~~~

That is B14 territory, not required to establish this B8 rejection, but it violates the handover law
that a check examining zero inputs must fail or explicitly be treated as unmeasured.

## Independent verifier evidence

Required command, run exactly:

~~~text
FLEET_MUTANTS=0 bash verify.sh
~~~

Observed output before interruption:

~~~text
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
  .... corpus
~~~

The command produced no final summary after approximately 10.5 minutes in corpus; I stopped the
process and captured exit 130. Therefore the exact required command did not produce green evidence.

Supplemental bounded run, with only the detector timeout changed:

~~~text
FLEET_DETECTOR_TIMEOUT=1 FLEET_MUTANTS=0 bash verify.sh
~~~

~~~text
  FAIL corpus (see var/verify.log)
-- 14 passed, 1 failed, 1 skipped (denominator: 16 stages) --
exit 6
~~~

The corpus stage itself reported:

~~~text
DENOMINATOR checked=34 total=34 excluded=69 caught=27
exit 1
~~~

This is the existing 103-detector corpus, not the required M9 detector. tests/corpus/M9.sh is absent.
FLEET_MUTANTS=0 also explicitly skips the general mutation stage, so the verifier result contains no
evidence that an M9 mutation was caught.

## Findings

### F1 — The cited empty-task command is not a reproducible test

Failure -> Cause -> Fix: fleet run --task "" returns a useful --repo is required message, not the
claimed silent empty-task result -> the command omits required --repo and --agent arguments -> publish
the complete valid-argument command and its exact isolated inputs, then fix and verify that path.

### F2 — The three user-facing defects remain, plus another silent mismatch path

Failure -> Cause -> Fix: valid empty task, tampered ledger, and unset-state plan still expose silent
or partial output; attest verify is also silent on mismatch -> typed exit codes are emitted without
consistently writing operator diagnostics, and plan prints fields before state validation -> validate
required state before rendering and require a reason line on every reachable non-zero exit, including
attestation mismatch.

### F3 — B8 has no M9 detector, denominator, or mutation proof

Failure -> Cause -> Fix: tests/corpus/M9.sh is absent and the walkthrough publishes no checked/total
for its proposed detector -> the acceptance item was documented as a recommendation, not implemented
evidence -> add M9 for every reachable refusable surface, fail on zero inputs, publish
checked/total/excluded/caught, add it to the integrity-covered corpus, and show a mutation that turns
the detector red before restoring it.

### F4 — The required gate is not green

Failure -> Cause -> Fix: exact FLEET_MUTANTS=0 bash verify.sh ended without a summary at exit 130;
the bounded reproducible run exits 6 with a red corpus stage -> the corpus contains timeouts and 27
caught findings, while mutants are skipped -> fix the red corpus/verifier path, rerun the exact
command to a final summary and exit 0, and separately publish the M9 mutation result.

### F5 — End to end is asserted without replayable proof

Failure -> Cause -> Fix: the accepted run did not produce an artifact in the shared checkout and
status reported 0 of 0 despite four ledger receipts -> the document gives historical prose rather
than a disposable repository fixture and per-command transcript -> publish exact setup, inputs,
artifact and attestation ids, status and ledger denominators, tamper operation, restore operation,
and exit/stream results; do not call the flow end to end until a clean run actually emits and verifies
an artifact.

## Required changes before acceptance

1. Fix the valid empty-task, tampered-ledger, and unset-state plan paths so each non-zero exit emits
   a human-readable reason and plan emits no partial fields before an environment refusal.
2. Add tests/corpus/M9.sh covering every reachable refusable surface, with a non-zero published
   checked/total denominator and explicit classification of excluded cases. Mutation-test the
   detector in both directions.
3. Make the corpus/verifier red state green, then run the exact FLEET_MUTANTS=0 bash verify.sh to a
   final summary with exit 0. Paste that complete output, including any skipped stage and its reason.
4. Replace the historical walkthrough claim with a replayable transcript containing all inputs,
   isolated state/repository setup, exit codes, stdout/stderr byte counts, artifact and SOW ids,
   status and ledger counts, tamper edit, and restoration command.
5. Track the fresh-store status --json checked=0 total=0 behavior separately under B14; do not
   present it as a clean measured status.
