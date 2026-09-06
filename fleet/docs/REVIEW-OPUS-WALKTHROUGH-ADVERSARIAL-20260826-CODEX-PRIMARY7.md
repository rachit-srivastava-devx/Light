# Adversarial review: opus walkthrough

Review ID: `OPUS-WALKTHROUGH-ADVERSARIAL-20260826-CODEX-PRIMARY7`  
Reviewed: `docs/delta.d/opus-walkthrough.md` on 2026-08-26  
Binary exercised: `keel/target/release/fleet`, version `0.1.0`, build `2026-08-24T18:42:07Z`  
Git commands run: none. Deliverable, `DELTA.md`, `BACKLOG.md`, and `keel/` were not edited.

## Verdict

**REJECT**

The walkthrough records real defects, but it is not reproducible acceptance evidence. Its literal
empty-task command does not produce the claimed output, the successful run cannot be replayed from
the information supplied, the counts are incomplete, and the nearest binding contract B8 remains
unchecked. The independent verifier is red.

## Contract check

There is no standalone `opus-walkthrough` item in `handover/BACKLOG.md`. The only reference is B8,
“Every non-zero exit must print a reason,” at lines 92–106. B8 requires all three paths to be fixed,
`tests/corpus/M9.sh` to exist with a denominator and mutation test, and `verify.sh` to be green.

Observed contract state:

- `tests/corpus/M9.sh`: missing.
- `tests/corpus/*.sh`: 105 files.
- The three paths are not all fixed: complete empty-task and tampered-ledger invocations remain
  silent.
- `verify.sh`: red, reported below.

## What the artifact claims

The artifact claims that a full `plan` → SOW → `run` → `status` → ledger verification → tamper →
restore flow worked end to end, that the SOW refusal was actionable, and that it found two silent
defects plus a third partial-output defect.

## What I actually ran

All runtime cases used disposable `mktemp -d` state. The first replay used:
`/Users/rachitsrivastava/youtube/Principal Engineering/fleet-rs/keel/target/release/fleet`.
The table reports captured stdout/stderr byte counts and process exit codes.

| Case | Exit | stdout / stderr | Observation |
|---|---:|---:|---|
| `plan "add a --version flag to the cli"` | 0 | 499 / 0 | Valid plan printed, but route was unavailable at stage 4. |
| `plan "make me a sandwich"` | 7 | 0 / 154 | Refusal named three closest intents. |
| `run --task "add a --version flag" --repo "$PWD" --agent stub` before SOW | 7 | 0 / 290 | Actionable SOW-id recovery text printed. |
| `sow --task "add a --version flag"` | 7 | 0 / 806 | Refusal named the missing citation and printed a template. |
| Complete multiline SOW from the template | 9 | 1502 / 194 | SOW was created; id `fcea10dfc63d8f375525606973ef28bb52c875b0cc333ea687b5897fdd51b99f`. |
| `sow accept --id <id>` | 0 | 117 / 0 | Acceptance recorded. |
| `run --task <accepted-task> --repo "$PWD" --agent stub` | 7 | 0 / 519 | Refused because the shared target repo had uncommitted changes; the artifact gives no clean-target precondition. |
| `status` after this flow | 0 | 2482 / 0 | Reported `1 of 1` receipt task as failed; no completed end-to-end run. |
| `ledger verify` before tamper | 0 | 27 / 0 | Printed `verified checked=5 total=5`, not the artifact’s unexplained `12/12`. |
| Tampered `ledger/chain.jsonl`; `ledger verify` | 8 | 0 / 0 | Silent mismatch reproduced. |
| Restored original chain; `ledger verify` | 0 | 27 / 0 | Printed `verified checked=1 total=1`. |
| Literal `run --task ""` | 7 | 0 / 187 | Did **not** reproduce “zero bytes”; it printed missing-`--repo` usage. |
| Complete `run --task "" --repo "$PWD" --agent stub` | 7 | 0 / 0 | The intended silent empty-task refusal reproduced and wrote one receipt. |
| Valid `plan` with `FLEET_STATE` unset | 3 | 55 / 264 | Printed `intent:`, `agent:`, and `skills:` before the environment fault. |
| Fresh `ledger verify` | 6 | 0 / 0 | Additional silent non-zero path omitted by the artifact. |
| Fresh `status --json` | 0 | 452 / 0 | Returned `checked: 0`, `total: 0`, `empty: true`: vacuous green state. |
| Fresh `attest verify 0094d2237b98a12d` | 8 | 0 / 0 | Additional silent mismatch path omitted by the artifact. |

The successful run claim is therefore not independently demonstrated. A SOW was accepted, but the
only target supplied by the checkout was dirty and the run stopped before producing an artifact.

## Findings

### F1 — The cited empty-task reproduction is wrong

`docs/delta.d/opus-walkthrough.md:11` cites `fleet run --task ""` and says it exits 7 silently.
As written, the command exits 7 with 187 bytes of missing-`--repo` usage. The silent behavior only
appears after adding the required `--repo` and `--agent` arguments. This matters because the artifact
claims a user replayed a specific command, but that command exercises a different validation branch.

### F2 — The end-to-end success claim is not reproducible

`docs/delta.d/opus-walkthrough.md:3-6` supplies no exact task, SOW body, SOW id, state directory,
binary identity, target repository, clean-target precondition, or captured output. Replaying the
closest documented flow stopped at `TARGET_REPO_NOT_CLEAN` with exit 7. “Works end to end” is not
established by the prose.

### F3 — The arithmetic is not a denominator

The artifact says “Two defects” at lines 8–14 and then adds “A third” at lines 16–18. That is three
reported cases, not two. It publishes no `checked/total` for the walkthrough, no count of refusal
surfaces considered, and no list of excluded or untriggerable hard cases. The `12/12` value is a
ledger-row count in an example, not walkthrough coverage.

The missing hard cases are observable: fresh `ledger verify` is silent with exit 6, fresh invalid
attestation verification is silent with exit 8, and fresh `status --json` returns exit 0 for
`checked=0,total=0`.

### F4 — The “code is right” conclusion is false for the complete empty-task path

The complete empty-task invocation exits 7, writes a refusal receipt, and emits no human-readable
reason. That is not merely a missing test-layer sentence; it is the live CLI behavior. The current
P0 test also discards both streams for this case (`tests/acceptance/p0.sh:108-113`), so its green
exit-code assertion cannot prove the user-facing property.

### F5 — The nearest acceptance contract is unmet

B8 remains unchecked. `M9.sh` is absent, no M9 reference appears in `verify.sh` or `tests/corpus`,
and the required verifier is red. A green targeted P0 run does not satisfy B8.

## Independent gate output

Command:

```text
FLEET_MUTANTS=0 bash verify.sh
```

Captured final output, exit code **6**:

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
  SKIPPED WITH A REASON mutants            (set FLEET_MUTANTS=1 - full pass ~24min)
  ok   attest-smoke
  ok   pytest
  ok   detectors
  FAIL corpus                     (see var/verify.log)
-- 14 passed, 1 failed, 1 skipped (denominator: 16 stages) --
```

For comparison, `bash tests/acceptance/p0.sh` exited 0 and printed `== 34 passed, 0 failed ==`,
but its F1 empty-task check suppresses stdout and stderr and asserts only exit 7 plus receipt growth.

## Required changes for acceptance

1. Restore an explicit `opus-walkthrough` acceptance item, or explicitly bind this artifact to B8
   and state that mapping.
2. Rewrite the walkthrough as a self-contained transcript: quote the repo path, state setup, exact
   task/SOW input, binary/version, clean-target precondition, every command, exit code, stdout,
   stderr, and the tamper/restore command.
3. Correct the empty-task command to include `--repo` and `--agent`, while recording that the
   literal shorthand only reaches missing-argument usage.
4. Publish a walkthrough `checked/total` denominator. Count all three defects as three, classify
   fresh/empty and untriggerable surfaces, and distinguish ledger-row denominators from case
   coverage.
5. If B8 is the contract, fix the three user-facing paths, add `tests/corpus/M9.sh` with a
   non-vacuous denominator, mutation-test both refusal and diagnostic directions, and rerun the
   full verifier to exit 0 with its final summary.
