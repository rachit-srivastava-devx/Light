# Adversarial review: opus walkthrough

Review ID: `OPUS-WALKTHROUGH-ADVERSARIAL-20260826-CODEX-FINAL`

Reviewed `docs/delta.d/opus-walkthrough.md` on 2026-08-26. No Git command was run. I did not edit
the deliverable, `docs/DELTA.md`, `handover/BACKLOG.md`, or `keel/`.

## Contract and verdict

There is no standalone `opus-walkthrough` heading in `handover/BACKLOG.md`. The deliverable is
explicitly referenced by B8 at `handover/BACKLOG.md:92-106`, so I used B8 as the contract. B8
requires the three paths fixed, `tests/corpus/M9.sh` with a published denominator and mutation
evidence, and a green verifier.

**REJECT**

The walkthrough records genuine defects, but B8 is not met. The defects remain reachable, M9 is
absent, mutation testing is not evidenced, and the required verifier is red.

## What was claimed

The deliverable claims:

1. A full plan/refusal/SOW/accept/run/status/ledger-verify/tamper/restore flow worked end to end.
2. `fleet run --task ""` returned exit 7 with zero bytes on both streams.
3. A tampered `fleet ledger verify` returned exit 8 with no output, while clean verification printed
   `verified checked=12 total=12`.
4. An unset `FLEET_STATE` made `fleet plan` print `intent:`, `agent:`, and `skills:` before the
   environment fault.
5. A future detector should require a reason for every non-zero exit.

The document publishes no exact state path, repository/task/SOW inputs, tamper operation, per-command
transcript, or walkthrough denominator.

## Commands actually run

The literal `fleet` command was not installed in this shell:

```text
fleet run --task ""                         rc=127, stdout=0 B, stderr=37 B
fleet ledger verify                          rc=127, stdout=0 B, stderr=37 B
```

I then ran the checkout binary at `./keel/target/debug/fleet` with isolated temporary state:

| Case | Exit | stdout | stderr | Observation |
|---|---:|---:|---:|---|
| `run --task "" --repo "$PWD" --agent stub` | 7 | 0 B | 0 B | Silent valid-argument empty-task refusal reproduced. |
| `ledger verify` on fresh state | 6 | 0 B | 0 B | Silent empty-ledger invariant failure, not covered by the walkthrough. |
| `plan "add a --version flag"` with `FLEET_STATE` unset | 3 | 55 B | 264 B | Partial plan printed before the environment diagnostic. |
| `plan` with no task | 7 | 0 B | 41 B | Usage diagnostic printed. |
| unsupported `plan "make me a sandwich"` | 7 | 0 B | 154 B | Refusal names three candidate intents. |
| run without accepted SOW | 7 | 0 B | 290 B | Refusal names the SOW id and recovery commands. |
| incomplete `sow --task "add a --version flag"` | 7 | 0 B | 806 B | Refusal names the missing citation and prints a corrected template. |

For an isolated 12-row ledger, all 12 appends returned 0. The clean/tamper/restore sequence was:

```text
clean verify:    rc=0, stdout=29 B, stderr=0 B, verified checked=12 total=12
tampered verify: rc=8, stdout=0 B,  stderr=0 B
restored verify: rc=0, stdout=29 B, stderr=0 B, verified checked=12 total=12
```

The README acceptance flow was also run:

```text
bash tests/acceptance/readme.sh
  ok   the README quickstart runs end to end (denominator: 31 lines)
rc=0
```

This proves the README fixture flow, not the undocumented full sequence claimed in the walkthrough.

## Independent acceptance checks

The required detector command was run exactly:

```text
bash tests/corpus/M9.sh
bash: tests/corpus/M9.sh: No such file or directory
rc=127
```

The required verifier was run exactly with `FLEET_MUTANTS=0`:

```text
FLEET_MUTANTS=0 bash verify.sh
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
verify_rc=6
```

The failed corpus log published:

```text
TIMEOUT A1.sh, A3.sh, A4.sh, A8.sh, B10.sh, C1.sh, C11.sh, C12.sh,
C19.sh, C2.sh, C22.sh, C24.sh, C26.sh, C3.sh, C6.sh, C8.sh, C9.sh,
S4.sh, S6.sh, S9.sh, T1.sh, T15.sh, T20.sh, T5.sh, T6.sh
DENOMINATOR checked=34 total=34 excluded=69 caught=27
```

There are 105 shell files in `tests/corpus`; subtracting `run.sh` and `_selftest.sh` leaves 103
detector candidates. The existing corpus denominator of `34 checked + 69 excluded = 103` is
arithmetically consistent, but it is not an M9 denominator and does not prove refusable-surface
coverage. `FLEET_MUTANTS=0` explicitly skips mutation testing.

## Findings

### F1 — The three documented defects remain in the implementation

**Failure → Cause → Fix:** the valid empty-task and tampered-ledger paths still return non-zero with
zero bytes, and unset-state planning still emits three lines first → error paths do not consistently
emit operator diagnostics → fix all three while preserving typed exit codes, and validate required
state before emitting plan output.

The source matches the runtime: `run_with_evidence` records `EMPTY_TASK` and returns 7 without a
diagnostic at `keel/fleet/src/main.rs:904-912`; `ledger_verify` propagates `verify_rows` before its
success-only print at `keel/fleet/src/main.rs:2995-3000`; `plan_command` prints the plan fields at
`keel/fleet/src/main.rs:2659-2661` before calling `state_dir()` at line 2664.

### F2 — The proposed detector is absent and the contract denominator is unpublished

**Failure → Cause → Fix:** `bash tests/corpus/M9.sh` returns 127 because the file does not exist →
the proposed rule was never made executable → add M9 over every reachable refusable surface, publish
`checked/total` and explicit exclusions, and fail when `checked=0`.

The additional probes found silent non-zero surfaces not listed in the walkthrough: invalid
`attest verify <id>` (`rc=8`, 0/0), missing attestation id (`rc=7`, 0/0), and invalid `ledger`
subcommand (`rc=7`, 0/0). M9 must classify these rather than quietly test only the three examples.

### F3 — Mutation evidence is missing

**Failure → Cause → Fix:** the required verifier run reports `mutants` skipped and no M9 exists →
there is no evidence the reason detector catches broken and non-broken controls → mutation-test M9
in both directions and publish the real caught/total result.

### F4 — The full-flow claim is not reproducible from the document

**Failure → Cause → Fix:** the document gives a date and arrow sequence but omits exact inputs,
state, repository fixture, artifact/SOW identifiers, tamper edit, and per-command exit/output → a
reviewer cannot independently replay or reconcile the claim → replace the prose with a transcript
and a numbered `checked/total` table. The README acceptance flow passing is not proof of the separate
undocumented plan/refusal/tamper sequence.

### F5 — Empty-store status passes vacuously (related B14 finding)

```text
FLEET_STATE=<fresh> ./keel/target/debug/fleet status --json
rc=0
{ "checked": 0, "total": 0, "empty": true, ... }
```

This is outside the three B8 examples but violates the repository law that an unmeasured check must
not look like a clean result. B14 already tracks it. Do not count this as evidence that B8 is fixed.

## Exactly what must change for ACCEPT

1. Fix the valid empty-task, tampered-ledger, and unset-state plan paths. Every reachable non-zero
   path must emit a user-facing line naming its reason, while retaining the typed exit code.
2. Add `tests/corpus/M9.sh` covering all reachable refusable and mismatch surfaces, including the
   extra silent cases found here. Publish a non-zero `checked/total` denominator and explicit
   exclusions; a zero-input run must fail.
3. Mutation-test M9 in both directions and publish caught/total evidence. Restore all seeded changes.
4. Replace the walkthrough's historical arrow sequence with a reproducible transcript containing
   exact commands, inputs, state/repo fixture, exit codes, stream output, artifact/SOW IDs, tamper
   operation, restoration, and a denominator.
5. Rerun `FLEET_MUTANTS=0 bash verify.sh` on a quiet checkout and require final exit 0 with no failed
   stage. Record the full final output, including every denominator and any intentional skip.
