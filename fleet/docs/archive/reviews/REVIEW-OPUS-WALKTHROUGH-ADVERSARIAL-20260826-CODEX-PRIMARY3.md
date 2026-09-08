# Adversarial review: opus walkthrough

Review ID: `OPUS-WALKTHROUGH-ADVERSARIAL-20260826-CODEX-PRIMARY3`

Reviewed: `docs/delta.d/opus-walkthrough.md` on 2026-08-26. No Git command was run. I did not edit
the deliverable, `docs/DELTA.md`, `handover/BACKLOG.md`, or `keel/`.

## Contract

There is no standalone `opus-walkthrough` item in `handover/BACKLOG.md`; the matching contract is
B8 at `handover/BACKLOG.md:94-106`. It requires the three user-facing failures to be fixed,
`tests/corpus/M9.sh` with a published denominator and mutation evidence, and a green verifier.

## Verdict

**REJECT**

The walkthrough records real defects, but it is not reproducible acceptance evidence and the B8
contract is not met. The valid empty-task path and tampered-ledger path are still silent, the plan
path still emits partial output, M9 is absent, mutation testing is absent, and the required gate is
red.

## What was claimed

The document claims:

1. A full `plan` → refusal → run → SOW refusal → SOW acceptance → run → status → ledger verify →
   tamper → restore flow works end to end.
2. `fleet run --task ""` exits `7` with zero bytes on both streams.
3. Tampered `fleet ledger verify` exits `8` with no output; the clean example prints
   `verified checked=12 total=12`.
4. Unset `FLEET_STATE` makes `fleet plan` print `intent:`, `agent:`, and `skills:` before its
   environment fault.
5. A detector should require a reason for every non-zero exit.

The document publishes no walkthrough `checked/total` denominator, exact state directory, exact
task/SOW inputs, repository fixture, tamper operation, or per-command transcript.

## What I actually ran

Binary: `keel/target/release/fleet`. Every invocation below captured stdout and stderr separately
and captured the exit code immediately.

| Command / case | Exit | stdout | stderr | Observation |
|---|---:|---:|---:|---|
| `FLEET_STATE=<tmp> fleet run --task ""` | 7 | 0 B | 187 B | **Does not reproduce the claimed empty-task case**; it refuses earlier with `--repo is required`. |
| `FLEET_STATE=<tmp> fleet run --task "" --repo "$PWD" --agent stub` | 7 | 0 B | 0 B | **Silent valid-argument empty-task refusal reproduced.** |
| `env -u FLEET_STATE fleet plan "add a --version flag"` | 3 | 55 B | 264 B | Prints the three plan lines before the environment-fault diagnostic. |
| `FLEET_STATE=<tmp> fleet plan "make me a sandwich"` | 7 | 0 B | 154 B | Refuses and names three close candidates. |
| `FLEET_STATE=<tmp> fleet status --json` on a fresh state | 0 | 452 B | 0 B | **Vacuous success:** `checked=0`, `total=0`, `empty=true`. |
| `FLEET_STATE=<tmp> fleet ledger verify` on a fresh state | 6 | 0 B | 0 B | Additional silent non-zero verification path omitted by the walkthrough. |
| `FLEET_STATE=<tmp> fleet attest verify 0094d2237b98a12d` | 8 | 0 B | 0 B | Another silent mismatch path omitted by the walkthrough. |
| `fleet run` before SOW acceptance, with the walkthrough task and current repo | 7 | 0 B | 290 B | Correctly refuses and prints the SOW id plus recovery commands. |
| SOW refusal: `fleet sow --task "add a --version flag"` | 7 | 0 B | 806 B | Names the missing citation and prints a corrected template. |
| Complete SOW creation with the README-shaped task | 9 | 1408 B | 194 B | Emits a SOW and id `417272d39922cb374f423c2b2094ca2cd57d8086d321c14d06ead49ee1c1a444`. |
| `fleet sow accept --id <that id>` | 0 | 125 B | 0 B | Acceptance receipt emitted. |
| Accepted `fleet run` against the shared checkout | 7 | 0 B | 519 B | Refused because the target repo has uncommitted changes; no artifact or successful run was observed. |

For an isolated clean ledger control, I appended exactly 12 rows:

```text
append_1=0 ... append_12=0
clean rc=0 stdout=29 stderr=0 text=verified checked=12 total=12
tampered rc=8 stdout=0 stderr=0
restored rc=0 stdout=29 stderr=0 text=verified checked=12 total=12
```

The success denominator is therefore arithmetically valid only for that separately controlled
12-row state. The walkthrough does not publish enough state to establish that its own `12/12`
came from exactly 12 rows rather than inherited receipts.

## Required detector and gate checks

`bash tests/corpus/M9.sh` exited **127**:

```text
bash: tests/corpus/M9.sh: No such file or directory
```

No M9 denominator or M9 mutation result exists. The existing corpus denominator is not a substitute
for M9's refusable-surface coverage.

The required independent command was run exactly:

```text
$ FLEET_MUTANTS=0 bash verify.sh
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

The corpus log at inspection reported `DENOMINATOR checked=34 total=34 excluded=69 caught=17`.
That is 34 + 69 = 103 classified detector scripts; it is not an M9 denominator. `FLEET_MUTANTS=0`
explicitly skipped mutation testing.

## Findings

### F1 — The cited empty-task command is incomplete

**Failure → Cause → Fix:** `fleet run --task ""` exits 7 with a useful `--repo is required`
message, not with the claimed silent empty-task result → the command omits required `--repo` and
`--agent` arguments → document and run the complete command, then fix and verify the actual
valid-argument empty-task path.

### F2 — The three user-facing defects remain

**Failure → Cause → Fix:** valid empty task, tampered ledger, and unset-state plan each still expose
the claimed silent/partial behavior → the failure paths return typed codes without consistently
writing operator diagnostics, and `plan` emits fields before `state_dir()` → emit a reason line
for every reachable non-zero path and validate required state before emitting plan content.

### F3 — The end-to-end success claim is not reproducible

**Failure → Cause → Fix:** no exact transcript or clean repository fixture is published, and the
accepted run could not complete in the shared checkout because it was dirty → the walkthrough
asserts a historical outcome without enough inputs to replay it → publish the exact disposable
state, repository fixture, task, SOW id, artifact id, status/ledger counts, tamper edit, restore
command, and exit/stream results; do not claim end-to-end success until a clean run produces an
artifact and attestation.

### F4 — No denominator for the walkthrough or “every non-zero exit” property

**Failure → Cause → Fix:** the document describes three cases but never states `N of M`, and the
fresh empty-ledger/attestation cases show the three cases are not exhaustive → omitted or
untriggerable surfaces are silently outside the count → add a numbered surface table with a
published denominator and explicit classifications.

### F5 — Required M9 and mutation evidence are absent

**Failure → Cause → Fix:** `tests/corpus/M9.sh` is missing and `FLEET_MUTANTS=0` skips mutation
testing → the “detector should” proposal has not become an executable acceptance gate → add M9 for
all reachable refusable surfaces, fail on zero checked inputs, publish its denominator, mutation-test
both reason and no-reason directions, and record the caught/total result.

### F6 — Required verifier is red

**Failure → Cause → Fix:** `FLEET_MUTANTS=0 bash verify.sh` exits 6 with 14/16 stages passed and
corpus failed → the acceptance contract requires green verification → resolve the corpus failure
and rerun to exit 0 with the final stage and corpus denominators pasted.

## Exactly what must change before acceptance

1. Correct the walkthrough command and replace the historical prose with a reproducible transcript
   containing all inputs, exit codes, stream byte counts, and artifacts.
2. Fix the valid empty-task, tampered-ledger, and unset-state plan paths; also account for silent
   empty-ledger and attestation-mismatch paths.
3. Add `tests/corpus/M9.sh` with a non-zero checked/total denominator and reason assertions for
   every reachable non-zero surface; update detector integrity if required by the repository law.
4. Mutation-test M9 in both directions and publish the real caught/total result. Do not treat the
   skipped mutants stage as mutation evidence.
5. Rerun `FLEET_MUTANTS=0 bash verify.sh` and require exit 0 with no failed stages. Explicitly
   identify B8 as the contract, or restore a standalone `opus-walkthrough` backlog item.

