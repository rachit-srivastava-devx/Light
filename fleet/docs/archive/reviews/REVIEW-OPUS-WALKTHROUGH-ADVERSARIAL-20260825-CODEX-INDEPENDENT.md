# Adversarial review: opus walkthrough

Reviewed `docs/delta.d/opus-walkthrough.md` against the matching B8 contract in
`handover/BACKLOG.md` (lines 92-106). No Git commands were run. The deliverable,
`DELTA.md`, `BACKLOG.md`, and `keel/` were not edited.

## What was claimed

- A complete plan/refusal/SOW/accept/run/status/ledger-verify/tamper/restore flow works end to end.
- `fleet run --task ""` exits 7 with zero output.
- Tampered `fleet ledger verify` exits 8 with no output; clean verification publishes `checked=12 total=12`.
- Unset `FLEET_STATE` causes `fleet plan` to print `intent:`, `agent:`, and `skills:` before its environment refusal.
- A detector should require every non-zero exit to print a reason.

## Commands actually run

All commands used the existing release binary at `keel/target/release/fleet` and isolated temporary
state directories where state was needed.

| Command/scenario | Exit | Observed result |
|---|---:|---|
| `FLEET_STATE=<tmp> fleet run --task ""` exactly as written | 7 | stdout 0 bytes; stderr 187 bytes saying `--repo is required`. This does not exercise empty-task validation. |
| `FLEET_STATE=<tmp> fleet run --task "" --repo "$PWD" --agent stub` | 7 | stdout 0 bytes; stderr 0 bytes. The intended empty-task silent refusal is real once required arguments are supplied. |
| `FLEET_STATE=<tmp> fleet plan` | 7 | stdout 0 bytes; stderr 41 bytes with usage. The walkthrough's partial-output claim needs a valid prompt. |
| `env -u FLEET_STATE fleet plan "add a --version flag to the cli"` | 3 | stdout 55 bytes containing exactly `intent:`, `agent:`, `skills:`; stderr 264 bytes naming the missing environment. Partial output reproduced. |
| append one ledger row | 0 | append succeeded. |
| clean `fleet ledger verify` | 0 | `verified checked=1 total=1`. |
| tamper ledger body, then `fleet ledger verify` | 8 | stdout 0 bytes; stderr 0 bytes. Silent mismatch reproduced. |
| restore saved ledger, then `fleet ledger verify` | 0 | `verified checked=1 total=1`. Restore path works. |
| `bash tests/acceptance/p0.sh` | 0 | `== 34 passed, 0 failed ==`; it checks the empty-task exit code but not its output. |
| `FLEET_MUTANTS=0 bash verify.sh` | 6 | `FAIL corpus`; `-- 14 passed, 1 failed, 1 skipped (denominator: 16 stages) --`. |

The corpus log from that run reported a non-vacuous corpus denominator of
`checked=34 total=34 excluded=69` and caught detector failures. The verifier therefore did not
finish green. The mutants stage was explicitly skipped under `FLEET_MUTANTS=0`, which is not
mutation evidence.

## Arithmetic and acceptance checks

- `docs/delta.d/opus-walkthrough.md` publishes no checked/total denominator for its walkthrough or
  its proposed detector.
- The corpus directory contains 103 detector shell scripts and 105 manifest lines, but only M1-M7
  exist among the M-series detectors. `tests/corpus/M9.sh` is absent.
- No M9 checked/total result or M9 mutation result is present. The existing `bin/mutants-gate.sh`
  cannot substitute for an M9-specific mutation test.
- B8 explicitly requires the three fixes, M9 with a published denominator, mutation testing, and a
  green verifier. None of those acceptance conditions is complete.

## Findings

1. **The walkthrough's first command is not reproducible as written.** `fleet run --task ""`
   reaches the required-argument usage path, not the empty-task path. The document must publish the
   required `--repo` and `--agent` arguments and an isolated setup. The underlying valid-argument
   empty-task refusal is still a real silent defect.
2. **Two non-zero user paths remain silent.** Valid empty-task refusal exits 7 with 0/0 bytes, and
   tampered-ledger verification exits 8 with 0/0 bytes. This directly violates B8's reason-output
   requirement.
3. **`fleet plan` emits partial output before an environment refusal.** The valid-prompt probe
   exits 3 after printing three plan lines. Either validate `FLEET_STATE` before rendering or make
   partial output an explicit, accepted contract. B8 currently says to fix the three paths.
4. **The required detector is missing.** `tests/corpus/M9.sh` does not exist, so every refusable
   surface is not being driven and no denominator proves coverage. A green P0 acceptance run does
   not satisfy this requirement.
5. **The required verifier is red.** The exact required command exited 6 with one failed stage.
   A claimed B8 completion cannot rely on a previous or partial green result.

## Verdict: REJECT

## Required changes for acceptance

1. Add user-facing reason lines for valid empty-task refusal and tampered-ledger mismatch; fix the
   partial `fleet plan` environment path as well.
2. Correct the walkthrough to use valid, reproducible commands and publish exact per-case exit and
   stdout/stderr results. Either provide reproducible artifacts for the claimed full flow or remove
   the end-to-end claim.
3. Add `tests/corpus/M9.sh` that drives every refusable surface, includes hard-to-trigger cases,
   fails on an untriggerable surface, names the reason for every non-zero exit, and publishes a
   non-zero `checked/total` denominator.
4. Mutation-test M9, capture both the detector-red and restored-green results, and publish the
   mutation denominator/result. Do not count the opt-in verifier skip as mutation evidence.
5. Rerun `FLEET_MUTANTS=0 bash verify.sh` after the fixes and record exit 0, zero failed stages,
   and the final corpus denominator.
