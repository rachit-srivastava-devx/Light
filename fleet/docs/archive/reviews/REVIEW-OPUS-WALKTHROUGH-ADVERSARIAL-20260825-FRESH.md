# Adversarial review: opus walkthrough

Date: 2026-08-25  
Artifact: `docs/delta.d/opus-walkthrough.md`  
Contract source: `handover/BACKLOG.md`

## Verdict: REJECT

The two silent failure defects are real, but the walkthrough is not an accurate,
reproducible user transcript. Its first cited command does not reproduce its own
claim, its successful end-to-end run is not evidenced in this checkout, and the
repository has no literal `opus-walkthrough` item in `handover/BACKLOG.md` to serve
as the stated contract.

## What was claimed

- The full `plan` / SOW / `run` / `status` / ledger-tamper flow works end to end.
- `fleet run --task ""` exits 7 with zero bytes on both streams.
- Tampered `fleet ledger verify` exits 8 with no output.
- An unset `FLEET_STATE` makes `fleet plan` print `intent`, `agent`, and `skills`
  before its environment refusal.
- A detector should check every non-zero exit for a reason, but no tested-surface
  denominator is published.

## Commands actually run

All CLI checks used `keel/target/release/fleet`; temporary state directories were
under `/tmp`. No deliverable, backlog, or DELTA file was changed.

| Command | Exit | Observed |
|---|---:|---|
| `FLEET_STATE=<tmp> fleet run --task ""` | 7 | **stdout 0 bytes, stderr 187 bytes**: `--repo is required` plus usage. The exact cited command does not reproduce the claimed silent failure. |
| `FLEET_STATE=<tmp> fleet run --task "" --repo "$PWD" --agent stub` | 7 | **stdout 0, stderr 0**. This reproduces the silent `EMPTY_TASK` failure once the required arguments and a valid Git repo are supplied. `ledger dump` independently showed a receipt with `reason=EMPTY_TASK`, `exit_code=7`. |
| `env -u FLEET_STATE fleet plan "add a --version flag to the cli"` | 3 | stdout 55 bytes: `intent:`, `agent:`, `skills:`. stderr 264 bytes explains the missing `FLEET_STATE`. Partial output is reproduced. |
| `fleet ledger append --event note --body '{"x":1}'` | 0 | One receipt appended. |
| clean `fleet ledger verify` | 0 | `verified checked=1 total=1`; stdout 27 bytes, stderr 0. |
| tampered `fleet ledger verify` | 8 | stdout 0, stderr 0. Silent mismatch is reproduced. |
| restored `fleet ledger verify` | 0 | `verified checked=1 total=1`. |

The broader pushback sequence also ran: `plan` accepted (0), sandwich `plan`
refused (7, useful nearest intents), run without SOW refused (7, 290 bytes),
incomplete SOW refused (7, 806 bytes with the missing citation and corrected
template), complete SOW produced `SOW_READY_AWAITING_REVIEW` (9), and SOW acceptance
succeeded (0). The claimed successful run did **not** complete: against this shared
checkout it returned 7 with `TARGET_REPO_NOT_CLEAN`, so no artifact was produced.
`status` then returned 0 and `ledger verify` returned 0 with `checked=5 total=5`;
tamper returned 8 with zero bytes and restore returned 0 with `checked=5 total=5`.

## Independent gates

Required targeted acceptance:

```text
$ bash tests/acceptance/p0.sh
...
== 34 passed, 0 failed ==
exit 0
```

Required independent full verifier:

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

The failing corpus run published:

```text
TIMEOUT A4.sh exceeded 30s -- treated as FAILED
TIMEOUT C11.sh exceeded 30s -- treated as FAILED
TIMEOUT C2.sh exceeded 30s -- treated as FAILED
TIMEOUT C22.sh exceeded 30s -- treated as FAILED
TIMEOUT C6.sh exceeded 30s -- treated as FAILED
TIMEOUT S6.sh exceeded 30s -- treated as FAILED
TIMEOUT S9.sh exceeded 30s -- treated as FAILED
TIMEOUT T1.sh exceeded 30s -- treated as FAILED
TIMEOUT T15.sh exceeded 30s -- treated as FAILED
M2: 32075 files in the tree (>15000). Build artifacts are almost certainly inside the repo;
    set CARGO_TARGET_DIR outside it. Detectors walk the tree and will stall (D30).
DENOMINATOR checked=34 total=34 excluded=69 caught=12
```

The stage arithmetic is checkable: `14 + 1 + 1 = 16`. The corpus denominator is
the scored set (`34 checked + 69 excluded`), not the 105-detector integrity count;
those are different populations and must not be presented as one result.

## Findings

### F1 — The primary repro command is incomplete and contradicts the claim

`docs/delta.d/opus-walkthrough.md:11` cites `fleet run --task ""` and says it
produces zero output. The literal command exits 7 with 187 bytes of usage output
because `--repo` is absent. The zero-byte behavior is a different path reached only
with `--repo "$PWD" --agent stub` and a valid Git repository.

Fix: cite the complete reproducible command, state the valid-repository precondition,
and distinguish the argument-refusal case from the empty-task case. Do not leave a
shorthand command that gives the user a different result.

### F2 — “Works end to end” is not supported by a reproducible transcript

`docs/delta.d/opus-walkthrough.md:3-6` claims the complete flow succeeds, but the
current replay stopped at `run` with exit 7 because the target checkout was dirty.
The document gives no exact task payload, binary path/version, state directory,
clean-tree precondition, command exit codes, or artifact ID for the claimed success.
An API-style or partial command sequence is not proof of a rendered/user-visible
working path.

Fix: either remove the end-to-end success claim or add a complete transcript from a
clean disposable repository, including each command, exit code, artifact ID, status
denominator, clean/tamper/restore results, and the exact binary used.

### F3 — The stated contract is absent

There is no literal `opus-walkthrough` entry in `handover/BACKLOG.md`. The only
matching reference is B8 at lines 92-105, which describes this fragment as evidence
for “Every non-zero exit must print a reason.” A reviewer cannot verify acceptance
criteria for an item that is not present.

Fix: add the named backlog item with explicit acceptance criteria, or state that B8
is the canonical contract and remove the inconsistent item name from the handover.

### F4 — The proposed “every non-zero exit” detector has no denominator

`docs/delta.d/opus-walkthrough.md:20-22` recommends testing every non-zero exit but
publishes neither the number of surfaces tested nor classifications for surfaces not
tested. This fails the project’s denominator law: “every” is an assertion, not a
measurement. The three observed cases are not evidence that all refusable surfaces
were covered.

Fix: enumerate the refusable command surfaces, publish `checked/total`, classify
each as explanatory or silent, and include the exit code and reason assertion for
each. Empty or untriggered surfaces must remain in the denominator.

## Additional adjacent defect found

On a fresh temporary `FLEET_STATE`, `fleet status --json` returned exit 0 with
`checked: 0` and `total: 0`. That is a vacuous clean result under the project’s own
law and is separately tracked by B14. It is not one of the three walkthrough claims,
but any “all failure paths” detector must not silently omit it.

## Required changes for acceptance

1. Correct F1’s command and preconditions.
2. Replace or substantiate F2 with a complete clean-target transcript.
3. Restore an explicit `opus-walkthrough` contract entry, or formally bind this
   artifact to B8.
4. Publish the detector’s tested-surface denominator and per-surface outcomes.
5. Re-run `FLEET_MUTANTS=0 bash verify.sh`; acceptance requires a final green result,
   not the targeted P0 result or partial stage output.
