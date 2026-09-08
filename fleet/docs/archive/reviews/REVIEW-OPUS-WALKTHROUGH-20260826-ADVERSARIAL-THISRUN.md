# Adversarial review: opus-walkthrough

Date: 2026-08-26  
Deliverable: `docs/delta.d/opus-walkthrough.md`  
Contract source: `handover/BACKLOG.md`

## Verdict: REJECT

The walkthrough records real silent paths, but it is not reproducible as written, does not prove its
end-to-end claim, omits the denominator required by the project rules, and proposes a detector that
does not exist. The B8 acceptance block is still open: the required verifier finished red.

There is no literal `opus-walkthrough` item in `handover/BACKLOG.md`. The only governing block is
B8, lines 92-106. B8 requires the three paths to be fixed, `tests/corpus/M9.sh` to exist with a
published denominator and mutation evidence, and `verify.sh` to be green.

## What was claimed

From `docs/delta.d/opus-walkthrough.md`:

1. A `plan` -> refusal -> `run` -> SOW refusal -> SOW acceptance -> `run` -> `status` -> ledger
   verify -> tamper -> restore flow works end to end.
2. `fleet run --task ""` exits 7 with zero bytes on stdout and stderr.
3. A tampered ledger verify exits 8 with no output, while the clean path prints
   `verified checked=12 total=12`.
4. Unset `FLEET_STATE` makes `fleet plan` print `intent:`, `agent:`, and `skills:` before the
   environment fault.
5. A detector should assert that every non-zero exit writes a line naming the reason.

## What I actually ran

I did not invoke git directly. The required verifier's child tests create temporary git fixtures.
The shell did not have a `fleet` command on PATH:

```text
command -v fleet                    -> exit 1
fleet --version                     -> exit 127, command not found
```

For product execution I prepended the checkout's release binary to PATH and set isolated
`FLEET_STATE` directories. The release and debug binaries produced the same results.

| Case | Exit | stdout | stderr | Observation |
|---|---:|---:|---:|---|
| Exact cited `fleet run --task ""` with state set | 7 | 0 B | 187 B | Refuses earlier because `--repo` is required; this is not the claimed silent empty-task branch. |
| Valid empty-task form: `fleet run --task "" --repo "$PWD" --agent stub` | 7 | 0 B | 0 B | Silent refusal reproduced. |
| `env -u FLEET_STATE fleet plan "add a --version flag"` | 3 | 55 B | 264 B | Prints the three plan fields, then the environment-fault diagnostic. |
| `fleet ledger verify` on a fresh isolated state | 6 | 0 B | 0 B | Silent invariant failure. |
| 12 valid ledger rows, clean verify | 0 | 29 B | 0 B | `verified checked=12 total=12`. |
| Same 12-row chain after changing one body field, verify | 8 | 0 B | 0 B | Silent mismatch reproduced. |
| Restored 12-row chain, verify | 0 | 29 B | 0 B | `verified checked=12 total=12`. |

The 12-row chain was created with the available `fleet ledger append --event note --body ...`
command in an isolated state, copied aside, tampered, verified, and restored. The output and exit
code were captured immediately for each invocation.

### End-to-end replay

Using the complete SOW shape from `README.md`/`tests/acceptance/swarm.sh` and the shared checkout
as the target, the replay was:

| Step | Exit | stdout | stderr | Observation |
|---|---:|---:|---:|---|
| `fleet plan "<complete task>"` | 0 | 499 B | 0 B | Plan rendered; route was unavailable but the plan completed. |
| `fleet run --task "<complete task>" --repo "$PWD" --agent stub` before SOW | 7 | 0 B | 290 B | Actionable `SOW_NOT_ACCEPTED` refusal. |
| `fleet sow --task "add a --version flag"` | 7 | 0 B | 806 B | Actionable missing-citation refusal and corrected template. |
| `fleet sow --task "<complete task>"` | 9 | 1604 B | 194 B | SOW and 64-character ID emitted. |
| `fleet sow accept --id <captured ID>` | 0 | 125 B | 0 B | `SOW_ACCEPTED` emitted. |
| Same accepted `fleet run` against `$PWD` | 7 | 0 B | 519 B | Refused because the shared checkout has uncommitted changes; no artifact or successful run. |

Therefore the claimed end-to-end success was not reproduced. The walkthrough gives no exact task,
state directory, binary identity, clean-target precondition, exit codes, artifact ID, or transcript
that would let another operator distinguish a successful run from a refusal.

## Arithmetic and hard-case audit

The walkthrough lists two primary defects plus one smaller defect, but publishes no denominator and
no checked/total result. The proposed `every non-zero exit` detector also has no denominator because
`tests/corpus/M9.sh` is absent:

```text
test -e tests/corpus/M9.sh              -> exit 1
M9                                      -> absent
```

An independent 14-case refusal-surface sweep found 7 silent non-zero exits and 7 cases with output
(denominator: 14). Silent cases were: valid empty task, empty ledger verify, empty ledger count,
attestation without an ID, bad attestation ID, ledger append without arguments, and impact without
a symbol. This is evidence that the walkthrough's three-case inventory is incomplete, not a full
detector denominator.

The same sweep found a vacuous success omitted by the walkthrough:

```text
fleet status --json on a fresh state -> exit 0, checked=0, total=0, empty=true
```

This violates the handover rule that a check examining zero inputs must fail or be explicitly
reported as unmeasured.

## Findings

### F1 — [P1] (confidence: 10/10): the cited empty-task command is incomplete

`docs/delta.d/opus-walkthrough.md:11` cites `fleet run --task ""`, but that invocation does not
reach empty-task validation. With state set it exits 7 with a 187-byte `--repo is required`
diagnostic; with state unset it is an environment failure. The silent 7/0/0 result requires the
omitted `--repo` and `--agent` arguments.

Failure -> Cause -> Fix: the cited command tests an earlier argument error -> required arguments
are omitted -> publish and execute the complete command with isolated state and binary identity.

### F2 — [P1] (confidence: 9/10): the end-to-end claim has no reproducible proof

`docs/delta.d/opus-walkthrough.md:3-6` provides no executable transcript or inputs. My replay got
through planning, SOW refusal, SOW creation, and acceptance, but the accepted run exited 7 on the
dirty shared checkout and produced no artifact. Calling this flow end to end is unsupported.

Failure -> Cause -> Fix: the successful run cannot be independently distinguished from a refused
run -> setup and outputs are absent and the target was not clean -> rerun on a clean target and
include exact commands, outputs, exit codes, artifact ID, status, tamper, and restore results.

### F3 — [P1] (confidence: 10/10): the proposed M9 acceptance evidence is missing

`docs/delta.d/opus-walkthrough.md:20-22` says the detector is mechanisable, but no
`tests/corpus/M9.sh` exists, the walkthrough publishes no detector denominator, and no mutation
test result is recorded. B8 explicitly requires all three.

Failure -> Cause -> Fix: the detector remains a recommendation rather than an implementation -> no
M9 file or mutation evidence is present -> add M9 over every reachable refusable surface, publish
checked/total, run the mutation test, and record the real result.

### F4 — [P1] (confidence: 10/10): the document calls the defect a surface gap while the code still
returns silent failures

`keel/fleet/src/main.rs:904-912` appends the empty-task refusal and returns exit 7 without printing;
`main.rs:2995-3000` propagates ledger verification errors before the success `println!`. The
observed behavior is a product defect in the user-facing command path, not only a test-coverage
gap. The same pattern remains in other reachable commands found by the 14-case sweep.

Failure -> Cause -> Fix: error paths return typed codes without a reason line -> diagnostics are
only emitted on selected branches -> make every reachable non-zero path emit a reason and test both
success and failure directions.

### F5 — [P2] (confidence: 10/10): hard cases are quietly omitted

The walkthrough does not classify empty-ledger verify/count, bad attestation, missing ledger append
arguments, missing impact symbol, or empty-state status. The latter exits 0 with `checked=0,total=0`,
which is a vacuous success. These cases prevent the three listed examples from being a complete
"every non-zero exit" inventory.

## Required changes for a REJECT

1. Add a literal `opus-walkthrough` backlog item, or explicitly declare B8 as its governing
   contract. Make the acceptance source unambiguous.
2. Correct the empty-task reproduction to include `--repo` and `--agent`; publish the exact binary,
   state setup, command, stdout/stderr bytes, and exit code.
3. Remove the end-to-end success claim until it is replayed on a clean target with a successful
   artifact-producing run and complete tamper/restore transcript.
4. Implement B8, including the three fixes, `tests/corpus/M9.sh`, its checked/total denominator,
   mutation evidence, and coverage for the additional reachable silent surfaces.
5. Rerun `FLEET_MUTANTS=0 bash verify.sh` and do not claim acceptance until the final summary is
   green.

## Required verifier evidence

Exact command:

```text
FLEET_MUTANTS=0 bash verify.sh
```

Final output:

```text
  FAIL corpus                     (see var/verify.log)
-- 14 passed, 1 failed, 1 skipped (denominator: 16 stages) --
VERIFY_EXIT=6
```

The corpus log published `DENOMINATOR checked=34 total=34 excluded=69 caught=27`; detector
timeouts were treated as failures. This is red evidence, not an acceptance result.
