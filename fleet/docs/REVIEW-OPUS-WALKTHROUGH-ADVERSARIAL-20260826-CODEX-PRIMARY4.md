# Adversarial review: opus walkthrough

Review ID: `OPUS-WALKTHROUGH-ADVERSARIAL-20260826-CODEX-PRIMARY4`
Date: 2026-08-26
Contract: `handover/BACKLOG.md` B8, lines 92-106

## What was claimed

`docs/delta.d/opus-walkthrough.md` claims that a complete plan/SOW/run/status/ledger/tamper/restore
flow works end to end, and reports three user-facing defects:

- `fleet run --task ""` exits 7 with zero bytes on both streams.
- A tampered `fleet ledger verify` exits 8 with no output; the clean example is
  `verified checked=12 total=12`.
- An unset-state `fleet plan` prints `intent:`, `agent:`, and `skills:` before its environment
  refusal.

The contract does not accept observation-only documentation. It requires fixing all three, adding
`tests/corpus/M9.sh` with a published denominator, mutation-testing it, and a green `verify.sh`.

## What I actually ran

All runs used the existing release binary at
`keel/target/release/fleet` and isolated temporary `FLEET_STATE` directories. No Git commands were
run.

| Command or path | Exit | stdout / stderr | Observation |
|---|---:|---:|---|
| `fleet run --task ""` with no `FLEET_STATE` | 3 | 0 / 64 bytes | Visible `MISSING_FLEET_STATE`; not the claimed `rc=7` silent path. |
| `FLEET_STATE=<tmp> fleet run --task ""` | 7 | 0 / 187 bytes | Visible `--repo is required`; still not the claimed silent path. |
| `FLEET_STATE=<tmp> fleet run --task "" --repo "$PWD" --agent stub` | 7 | 0 / 0 bytes | The underlying valid-argument empty-task silence reproduces. |
| `env -u FLEET_STATE fleet plan "add a --version flag"` | 3 | 55 / 264 bytes | The three partial plan lines precede the environment diagnostic. |
| Tampered `FLEET_STATE=<tmp> fleet ledger verify` | 8 | 0 / 0 bytes | Silent mismatch reproduces. |
| Restored `fleet ledger verify` | 0 | 27 / 0 bytes | `verified checked=2 total=2`; the observed denominator was 2, not 12. |
| Valid SOW creation | 9 | 1502 / 194 bytes | SOW was created awaiting review. |
| SOW acceptance | 0 | 125 / 0 bytes | Acceptance succeeded. |
| Accepted `fleet run` against the shared checkout | 7 | 0 / 519 bytes | Refused because the target repository has uncommitted changes; no artifact or successful run was observed. |
| Fresh `FLEET_STATE=<tmp> fleet status --json` | 0 | 452 / 0 bytes | Returned `checked=0`, `total=0`, `empty=true`: a vacuous successful status. |
| Fresh `FLEET_STATE=<tmp> fleet ledger verify` | 6 | 0 / 0 bytes | Another silent non-zero path, omitted by the walkthrough. |

The rest of the hand-driven path did produce visible refusals: an unmatched plan returned 7 with
154 stderr bytes, run-before-SOW returned 7 with 290 stderr bytes, and a malformed `fleet sow`
invocation returned 7 with 52 stderr bytes. Those do not prove the accepted run or the claimed
end-to-end artifact path.

## Independent gate

Command run exactly:

```text
FLEET_MUTANTS=0 bash verify.sh
```

Real terminal result:

```text
  FAIL corpus                     (see var/verify.log)
-- 14 passed, 1 failed, 1 skipped (denominator: 16 stages) --
```

The corpus log also reported:

```text
TIMEOUT A3.sh exceeded 30s -- treated as FAILED
TIMEOUT A4.sh exceeded 30s -- treated as FAILED
TIMEOUT A8.sh exceeded 30s -- treated as FAILED
TIMEOUT C11.sh exceeded 30s -- treated as FAILED
TIMEOUT C2.sh exceeded 30s -- treated as FAILED
TIMEOUT C22.sh exceeded 30s -- treated as FAILED
TIMEOUT C26.sh exceeded 30s -- treated as FAILED
TIMEOUT C6.sh exceeded 30s -- treated as FAILED
TIMEOUT S6.sh exceeded 30s -- treated as FAILED
TIMEOUT S9.sh exceeded 30s -- treated as FAILED
TIMEOUT T1.sh exceeded 30s -- treated as FAILED
TIMEOUT T15.sh exceeded 30s -- treated as FAILED
TIMEOUT T5.sh exceeded 30s -- treated as FAILED
DENOMINATOR checked=34 total=34 excluded=69 caught=17
```

`verify.sh` maps a failed required stage to exit 6. The command therefore exited 6. The mutant
stage was explicitly skipped because `FLEET_MUTANTS=0`; no mutation evidence exists for M9.

## Findings

1. **REJECT: the B8 contract is not implemented.** `tests/corpus/M9.sh` does not exist. The
   deliverable proposes that a detector “should assert” the behavior but does not provide the
   required detector, denominator, mutation result, or fixes. The backlog explicitly requires all
   three fixes, M9, mutation testing, and a green gate.

2. **REJECT: the literal empty-task claim is under-specified and false as a runnable command.**
   Without state, the cited command exits 3 with a diagnostic. With state, it exits 7 with a
   visible missing-`--repo` diagnostic. Only the fully specified command reaches the claimed silent
   empty-task refusal. The document must publish the actual environment, required arguments, binary,
   and per-command transcript.

3. **REJECT: the end-to-end success claim is unproven.** A valid SOW was created and accepted, but
   the subsequent run refused on the dirty shared checkout. No artifact, successful run, or
   12-row ledger was observed. `12=12` is arithmetically consistent but has no auditable setup or
   row breakdown; the reproducible clean chains here were `2/2` and `4/4`.

4. **Additional release-law finding: fresh status passes vacuously.** A fresh state returned
   `rc=0`, `checked=0`, `total=0`, `empty=true`. This violates the repository rule that a check
   examining zero inputs must fail. It is outside the three B8 fixes but must not be silently
   counted as evidence of a working status surface.

## Verdict

**REJECT**

## Exactly what must change

1. Fix the three B8 paths so every non-zero exit emits at least one user-readable reason: empty
   task, tampered ledger verification, and unset-state plan. Do not emit partial plan output before
   the environment refusal.
2. Add `tests/corpus/M9.sh` that drives every refusable surface, asserts a non-empty reason on every
   non-zero exit, skips no triggerable surface, and prints a denominator with checked/total and
   excluded counts.
3. Mutation-test M9 by breaking each watched behavior, show the detector red, restore it, and
   record the real mutation denominator. Run both the mutation gate and
   `FLEET_MUTANTS=0 bash verify.sh`; the latter must finish with `0` and a complete stage
   denominator, not the observed `14/1/1` result.
4. Re-run the full walkthrough against a disposable clean target and publish exact setup, inputs,
   stdout/stderr byte counts, exit codes, artifact ID, and ledger `checked/total` before tamper,
   after tamper, and after restore. Do not claim the 12-row result without that evidence.
5. Separately fix or explicitly reject the fresh `status --json` zero-input success; it must not
   be treated as measured evidence.
