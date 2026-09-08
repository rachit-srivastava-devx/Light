# Adversarial review: opus walkthrough

Date: 2026-08-25

Verdict: **REJECT**

## Contract

There is no standalone `opus-walkthrough` heading in `handover/BACKLOG.md`; the only reference is
the unchecked B8 item at `handover/BACKLOG.md:92-106`. Treating B8 as the nearest contract, its
acceptance requires all three paths to be fixed, `tests/corpus/M9.sh` to exist with a published
denominator, M9 to be mutation-tested, and `verify.sh` to be green.

## What was claimed

`docs/delta.d/opus-walkthrough.md` claims:

1. A full `plan`/SOW/`run`/status/ledger/tamper/restore flow worked end to end.
2. Two failure paths are silent: empty-task `run` and tampered-ledger `ledger verify`.
3. A third failure path prints three plan lines before the `FLEET_STATE` environment fault.
4. The exit codes are correct, and a detector should require a reason on every non-zero exit.

The arithmetic “two defects” plus “a third” is three defects, but the document publishes no
checked/total denominator for the claimed walkthrough or for the proposed detector.

## Commands actually run

All runs used `keel/target/release/fleet` by prepending `keel/target/release` to `PATH`, with
temporary `FLEET_STATE` directories. No repository files were used as state.

| Command | Exit | Observed |
|---|---:|---|
| `fleet run --task ""` | 7 | 0 stdout bytes, 187 stderr bytes; it refused for the missing required `--repo`, so the literal command in the document does **not** reproduce the claimed silent empty-task path. |
| `fleet run --task "" --repo "$PWD" --agent stub` | 7 | 0 stdout bytes, 0 stderr bytes; this fully specified empty-task path reproduces the silent failure. |
| `fleet plan "make me a sandwich"` | 7 | 0 stdout bytes, 154 stderr bytes; names three near matches. |
| `fleet run --task "add a --version flag"` | 7 | 0 stdout bytes, 290 stderr bytes; gives an actionable missing-SOW message. |
| `fleet sow --task "add a --version flag"` | 7 | 0 stdout bytes, 806 stderr bytes; names the missing challenge citation and prints a template. |
| `env -u FLEET_STATE fleet plan "add a --version flag"` | 3 | 55 stdout bytes containing `intent`, `agent`, and `skills`, then 264 stderr bytes explaining the missing state. The partial-output claim reproduces. |
| `fleet ledger verify` on a clean two-row receipt chain | 0 | `verified checked=2 total=2`. |
| `fleet ledger verify` after changing an actual ledger field | 8 | 0 stdout bytes, 0 stderr bytes. The tampered-chain claim reproduces. |
| `fleet ledger verify` after restoring the chain | 0 | `verified checked=2 total=2`. |
| `bash tests/corpus/M9.sh` | 127 | `No such file or directory`. M9 is absent; no detector denominator or mutation result exists. |

The intended three failure paths were all observed when the empty-task command was supplied with
the CLI arguments that its own usage requires: 3/3 reproduced. The document’s exact bare command
is under-specified and produces a different, actionable refusal.

## Independent verifier

Command:

```text
FLEET_MUTANTS=0 bash verify.sh
```

Exit: **6**

Real final output:

```text
  SKIPPED WITH A REASON mutants            (set FLEET_MUTANTS=1 - full pass ~24min)
  .... attest-smoke              ok
  .... pytest                    ok
  .... detectors                 ok
  .... corpus                    FAIL (see var/verify.log)
-- 14 passed, 1 failed, 1 skipped (denominator: 16 stages) --
```

`var/verify.log` identifies corpus timeouts including `A4.sh`, `C11.sh`, `C2.sh`, `C22.sh`,
`C6.sh`, `S6.sh`, `S9.sh`, `T1.sh`, and `T15.sh`; its final corpus denominator is
`checked=34 total=34 excluded=69 caught=12`.

## Implementation cross-check

- `keel/fleet/src/main.rs:2646-2712` prints the plan fields before calling `state_dir()`, which
  explains the partial plan output.
- `keel/fleet/src/main.rs:2995-3000` prints only after `verify_rows()` succeeds, which explains
  the silent mismatch.
- `keel/fleet/src/main.rs:842-846` prints only after `run_with_evidence()` succeeds; the fully
  specified empty task reaches a refusal path with no terminal diagnostic.

## Required changes before acceptance

1. Make the walkthrough reproducible: include the exact state/repository setup and the required
   `run` arguments, then publish a checked/total denominator for every exercised failure surface.
2. Fix all three user-facing paths so every non-zero result emits at least one line naming its
   reason: empty task, ledger mismatch, and partial-plan environment failure.
3. Add `tests/corpus/M9.sh` covering every refusable surface. It must publish `checked/total`, fail
   when it checks zero inputs, avoid matching its own documentation, and assert the reason text.
4. Mutation-test M9 in both directions, update detector integrity as required by B8, and record
   the mutation evidence.
5. Resolve the missing standalone `opus-walkthrough` contract, or explicitly declare B8 the
   contract, then rerun `FLEET_MUTANTS=0 bash verify.sh` to exit 0 with the final output recorded.

