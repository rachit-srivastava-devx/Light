# Adversarial review: opus walkthrough

Reviewed: `docs/delta.d/opus-walkthrough.md` on 2026-08-25.

## Verdict: REJECT

The two principal silent failures are real, but the deliverable is not acceptance evidence. The
named `opus-walkthrough` contract is absent, the nearest binding item B8 is still unchecked, M9 is
absent, the walkthrough has no coverage denominator, and the required verifier is red.

## What was claimed

The deliverable claims:

- a complete `plan` → SOW → `run` → `status` → ledger verify → tamper → restore flow worked
  (`docs/delta.d/opus-walkthrough.md:3-6`);
- `fleet run --task ""` returned exit 7 with zero bytes, and tampered `fleet ledger verify` returned
  exit 8 with no output (`:8-14`);
- `fleet plan` with `FLEET_STATE` unset emitted partial output before its environment refusal
  (`:16-18`);
- a detector should check every non-zero exit for a reason (`:20-22`).

`handover/BACKLOG.md` contains no standalone `opus-walkthrough` item. Its only occurrence is the
B8 reference at lines 92-96. Treating B8 as the nearest available contract, its acceptance at
lines 101-106 requires all three paths fixed, `tests/corpus/M9.sh` with a published denominator,
mutation testing, and a green `verify.sh`. B8 remains `[ ]`.

## What I ran

No git commands were run. I built the release binary with `cargo build --release` in `keel/`
(exit 0), then used a `mktemp -d` state directory so receipts did not enter the checkout.

| Command | Exit | Observed |
|---|---:|---|
| `env FLEET_STATE="$STATE" keel/target/release/fleet run --task ""` | 7 | 0 stdout bytes, 187 stderr bytes: missing `--repo` usage. This exact command as written does not reproduce the walkthrough's zero-byte claim. |
| `env FLEET_STATE="$STATE" keel/target/release/fleet run --task "" --repo "$PWD" --agent stub` | 7 | 0 stdout bytes, 0 stderr bytes. This repo-qualified form reproduces the silent empty-task defect. |
| `env -u FLEET_STATE keel/target/release/fleet plan "add a --version flag to the cli"` | 3 | 55 stdout bytes (`intent`, `agent`, `skills`) followed by a 264-byte stderr environment-fault explanation. The partial-output claim is reproduced. |
| `env -u FLEET_STATE keel/target/release/fleet plan` | 7 | 0 stdout bytes, 41 stderr bytes for usage. The walkthrough does not state the required prompt, so this adjacent form is not evidence for the partial-plan claim. |
| `env FLEET_STATE="$STATE" keel/target/release/fleet ledger verify` on a fresh state | 6 | 0 stdout bytes, 0 stderr bytes. This is an additional omitted silent non-zero surface. |
| `env FLEET_STATE="$STATE" keel/target/release/fleet sow --task "review the fleet walkthrough"` | 7 | Refusal was actionable and included the missing citation/template. |
| complete SOW via `fleet sow --task ...` | 9 | `SOW_READY_AWAITING_REVIEW` with an ID and acceptance command. |
| `fleet sow accept --id <id>` | 0 | `SOW_ACCEPTED` receipt printed. |
| `fleet run --task <task> --repo "$PWD" --agent stub` | 7 | Refused because the shared checkout has uncommitted changes; no successful run was claimed. |
| `fleet status` | 0 | `2 of 2 tasks from receipts`; the two failures were visible. |
| `fleet ledger verify` before tamper | 0 | `verified checked=7 total=7`. |
| Tamper one valid ledger body, then `fleet ledger verify` | 8 | 0 stdout bytes, 0 stderr bytes. This reproduces the silent tamper failure. |
| Restore the ledger, then `fleet ledger verify` | 0 | `verified checked=7 total=7`. |

The implementation matches the observed behavior: `run_with_evidence` records `EMPTY_TASK` and
returns without printing at `keel/fleet/src/main.rs:904-912`; `ledger_verify` only prints after
`verify_rows` succeeds at `keel/fleet/src/main.rs:2995-3000`; and `plan_command` prints the three
plan lines before calling `state_dir()` at `keel/fleet/src/main.rs:2659-2664`.

## Independent verifier

Exact command: `FLEET_MUTANTS=0 bash verify.sh`

Real output, including red stages:

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
  FAIL swarm                      (see var/verify.log)
  ok   policy
  SKIPPED WITH A REASON mutants            (set FLEET_MUTANTS=1 - full pass ~24min)
  ok   attest-smoke
  ok   pytest
  ok   detectors
  FAIL corpus                     (see var/verify.log)
-- 13 passed, 2 failed, 1 skipped (denominator: 16 stages) --
```

Exit code: `6`.

The available `var/verify.log` identifies the swarm failure as `CC1 six concurrent dispatches on
a cold store all succeed: 1 of 6 failed` while `CC2` and `CC3` passed. Its corpus run reports
`DENOMINATOR checked=34 total=34 excluded=69 caught=12`, but also treats these nine 30-second
timeouts as failures: `A4`, `C11`, `C2`, `C22`, `C6`, `S6`, `S9`, `T1`, and `T15`.

## Arithmetic and completeness findings

1. The walkthrough publishes no `checked/total` denominator for the flow or for refusal-surface
   coverage. The `checked=7 total=7` value above is ledger-row integrity, not walkthrough coverage.
2. “Two defects” plus “a third, smaller” adds to three observed cases, but the proposed property
   is every non-zero exit. The other surfaces are not enumerated, attempted, or classified as
   omitted/untriggerable. The fresh-ledger silent exit is one concrete omitted case.
3. `tests/corpus/M9.sh` does not exist. Therefore there is no detector denominator, no proof that
   the detector excludes its own documentation, and no mutation arm for this requirement.
4. The verifier's `13/16` result is not green. Mutants being explicitly skipped is not a pass, and
   the two red stages prevent B8 acceptance even apart from the missing M9.
5. No item is falsely marked complete in the reviewed deliverable, but the backlog status is the
   opposite of done: B8 is unchecked and the available progress log records verifier failures.

## Exactly what must change for acceptance

1. Add a real `opus-walkthrough` item to `handover/BACKLOG.md`, or explicitly bind this artifact to
   B8. Its acceptance text must be the source of truth; do not rely on an undocumented nearest-item
   inference.
2. Make the walkthrough reproducible: include the exact setup, quoted repo path, `--repo` and
   `--agent` arguments, SOW text/ID handling, stdout/stderr capture, exit codes, tamper edit, and
   restore command. Publish a walkthrough denominator and classify every omitted hard case.
3. Fix all three claimed paths so every non-zero exit emits at least one line naming its reason:
   empty task, tampered ledger verification, and unset-state planning. Preserve the receipt and
   typed exit code while adding the operator-facing reason.
4. Add `tests/corpus/M9.sh` covering every refusable surface, with both a measurable
   `checked/total` denominator and failure for any zero-input or untriggerable surface. Ensure it
   cannot pass by matching its own documentation.
5. Mutation-test M9 with a compiling mutation that removes or corrupts each reason assertion;
   show the mutated run red, restore the code, and update the detector integrity manifest if the
   repository's detector workflow requires it.
6. Resolve the current `swarm` and `corpus` failures, then rerun the exact
   `FLEET_MUTANTS=0 bash verify.sh` command to a final exit 0 with its final `16/16` stage
   denominator. Do not report acceptance from the current `13/16` result.

